use std::time;

error_chain::error_chain! {
    errors {
        EOF(expected:String, got:String, exit_code:Option<String>) {
            description("End of filestream (usually stdout) occurred, most probably\
                         because the process terminated")
            display("EOF (End of File): Expected {} but got EOF after reading \"{}\", \
                         process terminated with {:?}", expected, got,
                         exit_code.as_ref()
                         .unwrap_or(& "unknown".to_string()))
        }
        BrokenPipe {
            description("The pipe to the process is broken. Most probably because\
            the process died.")
            display("PipeError")
        }
        Timeout(expected:String, got:String, timeout:time::Duration) {
            description("The process didn't end within the given timeout")
            display("Timeout Error: Expected {} but got \"{}\" (after waiting {} ms)",
                    expected, got, (timeout.as_secs() * 1000) as u32
                    + timeout.subsec_millis())
        }
        EmptyProgramName {
            description("The provided program name is empty.")
            display("EmptyProgramName")
        }
    }
}
