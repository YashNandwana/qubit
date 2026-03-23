pub mod factory;
pub mod grpc;
pub mod handler;
pub mod http;

pub use factory::ServerFactory;
pub use http::HttpServer;

pub mod qubit {
    tonic::include_proto!("qubit");

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("qubit_descriptor");
}
