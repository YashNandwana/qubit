use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{DeleteParams, Patch, PatchParams, PostParams};
use kube::{Api, Client};

#[derive(Parser, Debug)]
#[command(name = "load-gen")]
struct Args {
    /// ConfigMap operations per second (create/patch/delete). Exercises cluster-agent → Core.
    #[arg(long, default_value = "0", env = "K8S_RPS")]
    k8s_rps: u32,

    /// HTTP requests per second to --http-target. eBPF loader captures these → Core.
    #[arg(long, default_value = "0", env = "HTTP_RPS")]
    http_rps: u32,

    #[arg(
        long,
        default_value = "http://service-b.default.svc.cluster.local/",
        env = "HTTP_TARGET"
    )]
    http_target: String,

    #[arg(long, default_value = "load-gen", env = "K8S_NAMESPACE")]
    k8s_namespace: String,

    #[arg(long, default_value = "60", env = "DURATION")]
    duration: u64,
}

#[derive(Default)]
struct Stats {
    sent: AtomicU64,
    errors: AtomicU64,
}

impl Stats {
    fn ok(&self) {
        self.sent.fetch_add(1, Ordering::Relaxed);
    }
    fn err(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }
    fn snapshot(&self) -> (u64, u64) {
        (
            self.sent.load(Ordering::Relaxed),
            self.errors.load(Ordering::Relaxed),
        )
    }
}

fn ticker(rps: u32) -> tokio::time::Interval {
    tokio::time::interval(Duration::from_secs_f64(1.0 / rps as f64))
}

// Cycles through create / patch / delete on a pool of ConfigMaps.
// Pool capped at 50 to avoid unbounded growth; cleaned up on exit.
async fn k8s_stream(
    namespace: String,
    rps: u32,
    duration: Duration,
    stats: Arc<Stats>,
) -> Result<()> {
    let client = Client::try_default().await?;
    let cms: Api<ConfigMap> = Api::namespaced(client, &namespace);

    let mut t = ticker(rps);
    let deadline = Instant::now() + duration;
    let mut tick = 0u64;
    let mut pool: Vec<String> = Vec::new();

    while Instant::now() < deadline {
        t.tick().await;

        let op = if pool.is_empty() {
            "create"
        } else {
            match tick % 10 {
                0..=5 => "patch",
                6..=7 => {
                    if pool.len() < 50 {
                        "create"
                    } else {
                        "patch"
                    }
                }
                _ => "delete",
            }
        };

        let result = match op {
            "patch" => {
                let name = pool[tick as usize % pool.len()].clone();
                let patch = serde_json::json!({ "data": { "tick": tick.to_string() } });
                cms.patch(&name, &PatchParams::default(), &Patch::Merge(patch))
                    .await
                    .map(|_| ())
            }
            "create" => {
                let name = format!("load-gen-{}", tick);
                let mut data = BTreeMap::new();
                data.insert("tick".to_string(), tick.to_string());
                let cm = ConfigMap {
                    metadata: ObjectMeta {
                        name: Some(name.clone()),
                        namespace: Some(namespace.clone()),
                        ..Default::default()
                    },
                    data: Some(data),
                    ..Default::default()
                };
                let r = cms.create(&PostParams::default(), &cm).await.map(|_| ());
                if r.is_ok() {
                    pool.push(name);
                }
                r
            }
            _ => {
                let name = pool.remove(0);
                cms.delete(&name, &DeleteParams::default())
                    .await
                    .map(|_| ())
            }
        };

        match result {
            Ok(_) => stats.ok(),
            Err(_) => stats.err(),
        }
        tick += 1;
    }

    for name in &pool {
        let _ = cms.delete(name, &DeleteParams::default()).await;
    }
    Ok(())
}

async fn http_stream(
    target: String,
    rps: u32,
    duration: Duration,
    stats: Arc<Stats>,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let paths = [
        "/",
        "/api/v1/users",
        "/api/v1/orders",
        "/api/v1/status",
        "/metrics",
    ];

    let mut t = ticker(rps);
    let deadline = Instant::now() + duration;
    let mut tick = 0usize;

    while Instant::now() < deadline {
        t.tick().await;
        let url = format!(
            "{}{}",
            target.trim_end_matches('/'),
            paths[tick % paths.len()]
        );
        match client.get(&url).send().await {
            Ok(_) => stats.ok(),
            Err(_) => stats.err(),
        }
        tick += 1;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.k8s_rps == 0 && args.http_rps == 0 {
        eprintln!("Error: at least one of --k8s-rps or --http-rps must be > 0");
        std::process::exit(1);
    }

    let duration = Duration::from_secs(args.duration);

    println!(
        "load-gen  duration={}s  k8s={}rps  http={}rps",
        args.duration, args.k8s_rps, args.http_rps
    );

    let k8s_stats = Arc::new(Stats::default());
    let http_stats = Arc::new(Stats::default());
    let mut tasks = Vec::new();

    if args.k8s_rps > 0 {
        let (ns, rps, dur, s) = (
            args.k8s_namespace.clone(),
            args.k8s_rps,
            duration,
            Arc::clone(&k8s_stats),
        );
        tasks.push(tokio::spawn(
            async move { k8s_stream(ns, rps, dur, s).await },
        ));
    }

    if args.http_rps > 0 {
        let (tgt, rps, dur, s) = (
            args.http_target.clone(),
            args.http_rps,
            duration,
            Arc::clone(&http_stats),
        );
        tasks.push(tokio::spawn(
            async move { http_stream(tgt, rps, dur, s).await },
        ));
    }

    // Print progress every 5s
    let k = Arc::clone(&k8s_stats);
    let h = Arc::clone(&http_stats);
    let (k8s_rps, http_rps) = (args.k8s_rps, args.http_rps);
    let reporter = tokio::spawn(async move {
        let start = Instant::now();
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.tick().await;
        loop {
            interval.tick().await;
            if k8s_rps > 0 {
                let (sent, err) = k.snapshot();
                println!(
                    "  [{:.0}s] k8s  {} ok  {} err",
                    start.elapsed().as_secs_f64(),
                    sent,
                    err
                );
            }
            if http_rps > 0 {
                let (sent, err) = h.snapshot();
                println!(
                    "  [{:.0}s] http {} ok  {} err",
                    start.elapsed().as_secs_f64(),
                    sent,
                    err
                );
            }
        }
    });

    for task in tasks {
        task.await??;
    }
    reporter.abort();

    let (k8s_sent, k8s_err) = k8s_stats.snapshot();
    let (http_sent, http_err) = http_stats.snapshot();
    let secs = args.duration as f64;

    println!("\n--- summary ---");
    if args.k8s_rps > 0 {
        println!(
            "  k8s   {} ok  {} err  ({:.0}/s)",
            k8s_sent,
            k8s_err,
            k8s_sent as f64 / secs
        );
    }
    if args.http_rps > 0 {
        println!(
            "  http  {} ok  {} err  ({:.0}/s)",
            http_sent,
            http_err,
            http_sent as f64 / secs
        );
    }

    Ok(())
}
