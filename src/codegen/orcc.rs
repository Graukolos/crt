pub const OPTIONS_H: &str = r"#ifndef OPTIONS_HEADER
#define OPTIONS_HEADER
struct ORCC_options {
	char *input_file;
	char *input_directory;
	char display_flags;
	int nbLoops;
	int nbFrames;
	char *yuv_file;
	char *mapping_input_file;
	char *mapping_output_file;
	int nb_processors;
	int enable_dynamic_mapping;
	int nbProfiledFrames;
	int mapping_repetition;
	char *profiling_file;
	char *write_file;
	int print_firings;
};
typedef struct ORCC_options options_t;
extern options_t *opt;
void parse_command_line_input(int argc, char *argv[]);
#endif
";

pub const OPTIONS_RS: &str = r"#[repr(C)]
struct OrccOptions {
    input_file: *mut core::ffi::c_char,
    input_directory: *mut core::ffi::c_char,
    display_flags: core::ffi::c_char,
    nb_loops: core::ffi::c_int,
    nb_frames: core::ffi::c_int,
    yuv_file: *mut core::ffi::c_char,
    mapping_input_file: *mut core::ffi::c_char,
    mapping_output_file: *mut core::ffi::c_char,
    nb_processors: core::ffi::c_int,
    enable_dynamic_mapping: core::ffi::c_int,
    nb_profiled_frames: core::ffi::c_int,
    mapping_repetition: core::ffi::c_int,
    profiling_file: *mut core::ffi::c_char,
    write_file: *mut core::ffi::c_char,
    print_firings: core::ffi::c_int,
}

impl OrccOptions {
    fn new() -> Self {
        Self {
            input_file: core::ptr::null_mut(),
            input_directory: core::ptr::null_mut(),
            display_flags: 1,
            nb_loops: -1,
            nb_frames: -1,
            yuv_file: core::ptr::null_mut(),
            mapping_input_file: core::ptr::null_mut(),
            mapping_output_file: core::ptr::null_mut(),
            nb_processors: 1,
            enable_dynamic_mapping: 0,
            nb_profiled_frames: 10,
            mapping_repetition: 1,
            profiling_file: core::ptr::null_mut(),
            write_file: core::ptr::null_mut(),
            print_firings: 0,
        }
    }
}

#[unsafe(no_mangle)]
static mut opt: *mut OrccOptions = core::ptr::null_mut();
";

pub const MAIN_SETUP: &str = r"    use clap::Parser;
    #[derive(Parser)]
    struct OrccArgs {
        #[arg(short = 'i')]
        input_file: String,
        #[arg(short = 'w')]
        write_file: String,
    }
    let cli = OrccArgs::parse();
    let mut __opt = Box::new(OrccOptions::new());
    __opt.input_file = std::ffi::CString::new(cli.input_file).unwrap().into_raw();
    __opt.write_file = std::ffi::CString::new(cli.write_file).unwrap().into_raw();
    unsafe { opt = Box::into_raw(__opt); }

";
