#[cxx::bridge(namespace = "netshim")]
pub mod ffi {
    #[derive(Debug, Clone)]
    pub struct Network {
        pub name: String,
        pub instances: Vec<Instance>,
        pub edges: Vec<Edge>,
        pub class_paths: Vec<ClassPath>,
    }

    #[derive(Debug, Clone)]
    pub struct Instance {
        pub id: String,
        pub class_name: String,
        pub parameters: Vec<Param>,
    }

    #[derive(Debug, Clone)]
    pub struct Param {
        pub key: String,
        pub value: String,
    }

    #[derive(Debug, Clone)]
    pub struct Edge {
        pub src_id: String,
        pub src_port: String,
        pub dst_id: String,
        pub dst_port: String,
        pub fifo_size: u32,
    }

    #[derive(Debug, Clone)]
    pub struct ClassPath {
        pub class_name: String,
        pub path: String,
    }

    unsafe extern "C++" {
        include!("cpp/network_shim.hpp");

        fn read_network(network_file: &str, source_dir: &str) -> Result<Network>;
    }
}
