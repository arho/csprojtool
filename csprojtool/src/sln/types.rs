pub enum Configuration {
    Debug,
    Release,
}

impl std::fmt::Display for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match *self {
            Self::Debug => "Debug",
            Self::Release => "Release",
        })
    }
}

pub const CONFIGURATIONS: [Configuration; 2] = [Configuration::Debug, Configuration::Release];

pub enum ProcessorArchitecture {
    Any,
    X64,
    X86,
}

impl std::fmt::Display for ProcessorArchitecture {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match *self {
            Self::Any => "Any CPU",
            Self::X64 => "x64",
            Self::X86 => "x86",
        })
    }
}

pub const PROCESSOR_ARCHITECTURES: [ProcessorArchitecture; 3] = [
    ProcessorArchitecture::Any,
    ProcessorArchitecture::X64,
    ProcessorArchitecture::X86,
];
