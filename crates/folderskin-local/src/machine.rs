//! The computer: what it has, and what that means for how the models run.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    Macos,
    Linux,
}

impl Os {
    pub fn this() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Macos
        } else {
            Os::Linux
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Os::Windows => "windows",
            Os::Macos => "macos",
            Os::Linux => "linux",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arch {
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "arm64")]
    Arm64,
}

impl Arch {
    pub fn this() -> Arch {
        if cfg!(target_arch = "aarch64") {
            Arch::Arm64
        } else {
            Arch::X86_64
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Arm64 => "arm64",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Gpu {
    Nvidia,
    Apple,
    Other,
    None,
}

/// What this computer has, as far as generation cares.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    pub os: Os,
    pub arch: Arch,
    pub ram_gb: f64,
    pub gpu: Gpu,
    pub gpu_name: String,
    /// Dedicated VRAM; on Apple Silicon, the unified memory; 0 when it isn't known.
    pub vram_gb: f64,
}

impl Machine {
    /// "windows x86_64, 32 GB RAM, NVIDIA GeForce RTX 3050 Ti Laptop GPU (4 GB)"
    pub fn describe(&self) -> String {
        let gpu = match self.gpu {
            Gpu::None => "no GPU found".to_string(),
            _ if self.vram_gb > 0.0 => format!("{} ({:.0} GB)", self.gpu_name, self.vram_gb),
            _ => self.gpu_name.clone(),
        };
        format!(
            "{} {}, {:.0} GB RAM, {gpu}",
            self.os.id(),
            self.arch.id(),
            self.ram_gb
        )
    }
}

/// What runs the models.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    /// stable-diffusion.cpp on an NVIDIA card, on Windows.
    Cuda,
    /// stable-diffusion.cpp on any GPU with a Vulkan driver: NVIDIA, AMD, Intel.
    Vulkan,
    /// stable-diffusion.cpp on a Mac's GPU. It wants macOS 26; mflux is the Mac's default.
    Metal,
    /// stable-diffusion.cpp on the processor: minutes a picture.
    Cpu,
    /// mflux on Apple Silicon, the MLX port of the same two models.
    Mlx,
}

impl Backend {
    pub const ALL: [Backend; 5] = [
        Backend::Cuda,
        Backend::Vulkan,
        Backend::Metal,
        Backend::Cpu,
        Backend::Mlx,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Backend::Cuda => "cuda",
            Backend::Vulkan => "vulkan",
            Backend::Metal => "metal",
            Backend::Cpu => "cpu",
            Backend::Mlx => "mlx",
        }
    }

    pub fn parse(s: &str) -> Option<Backend> {
        Backend::ALL.into_iter().find(|b| b.id() == s)
    }

    /// Whether stable-diffusion.cpp runs this backend (everything but mflux).
    pub fn is_sdcpp(self) -> bool {
        self != Backend::Mlx
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// How finely the model weights are quantised.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// 8-bit: the best, about 15.7 GB of downloads, wants ~24 GB of RAM.
    Q8,
    /// 4-bit: smaller and a little softer, about 9.4 GB.
    Q4,
}

impl Tier {
    pub fn id(self) -> &'static str {
        match self {
            Tier::Q8 => "q8",
            Tier::Q4 => "q4",
        }
    }

    pub fn parse(s: &str) -> Option<Tier> {
        [Tier::Q8, Tier::Q4].into_iter().find(|t| t.id() == s)
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// The backend for `m`: MLX on Apple Silicon, CUDA for NVIDIA on Windows, Vulkan for any other
/// GPU (no Linux CUDA build is published, and Vulkan runs on NVIDIA, AMD and Intel), else CPU.
pub fn pick_backend(m: &Machine) -> Backend {
    match (m.os, m.gpu) {
        (Os::Macos, _) if m.arch == Arch::Arm64 => Backend::Mlx,
        (Os::Macos, _) => Backend::Cpu,
        (Os::Windows, Gpu::Nvidia) => Backend::Cuda,
        (_, Gpu::Nvidia | Gpu::Other) => Backend::Vulkan,
        _ => Backend::Cpu,
    }
}

/// 8-bit weights where memory allows. They stream from RAM when they don't fit on the card, so
/// RAM decides, not VRAM: a 4 GB laptop GPU runs the 8-bit models.
pub fn pick_tier(m: &Machine) -> Tier {
    if m.ram_gb >= 24.0 {
        Tier::Q8
    } else {
        Tier::Q4
    }
}

/// Looks at this computer. Runs `nvidia-smi` and, failing that, asks the system for its display
/// adapters, so it can take a second or two; call it off the UI thread.
pub fn detect() -> Machine {
    let (os, arch, ram_gb) = (Os::this(), Arch::this(), ram_gb());
    let machine = |gpu, gpu_name: String, vram_gb| Machine {
        os,
        arch,
        ram_gb,
        gpu,
        gpu_name,
        vram_gb,
    };
    if os == Os::Macos {
        if arch == Arch::Arm64 {
            let chip = run_quiet("sysctl", &["-n", "machdep.cpu.brand_string"]);
            let chip = if chip.is_empty() {
                "Apple Silicon".into()
            } else {
                chip
            };
            return machine(Gpu::Apple, chip, ram_gb);
        }
        return machine(Gpu::None, String::new(), 0.0);
    }
    let smi = run_quiet(
        "nvidia-smi",
        &[
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ],
    );
    if let Some((name, mib)) = smi.lines().next().and_then(|l| l.rsplit_once(',')) {
        let vram = mib.trim().parse::<f64>().unwrap_or(0.0) / 1024.0;
        return machine(Gpu::Nvidia, name.trim().to_string(), vram);
    }
    let names = if os == Os::Windows {
        run_quiet(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_VideoController).Name",
            ],
        )
    } else {
        run_quiet(
            "sh",
            &["-c", "lspci 2>/dev/null | grep -Ei 'vga|3d|display'"],
        )
    };
    match real_adapters(&names).first() {
        // Windows reports AdapterRAM in 32 bits and Linux doesn't say; Vulkan's --auto-fit copes.
        Some(name) => machine(Gpu::Other, name.clone(), 0.0),
        None => machine(Gpu::None, String::new(), 0.0),
    }
}

/// Display adapters that can run a model: not Windows' basic display driver, not a virtual one.
fn real_adapters(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .filter(|n| {
            let lower = n.to_lowercase();
            !lower.contains("basic") && !lower.contains("virtual")
        })
        .map(str::to_string)
        .collect()
}

/// Total memory in GiB.
fn ram_gb() -> f64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        // SAFETY: MEMORYSTATUSEX is plain data; zeroed with its length set is what the call wants.
        let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        // SAFETY: `status` is a valid, writable MEMORYSTATUSEX for the length of the call.
        if unsafe { GlobalMemoryStatusEx(&mut status) } != 0 {
            return status.ullTotalPhys as f64 / (1u64 << 30) as f64;
        }
        0.0
    }
    #[cfg(target_os = "macos")]
    {
        run_quiet("sysctl", &["-n", "hw.memsize"])
            .parse::<f64>()
            .map_or(0.0, |bytes| bytes / (1u64 << 30) as f64)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|info| {
                info.lines()
                    .find_map(|l| l.strip_prefix("MemTotal:"))
                    .and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok())
            })
            .map_or(0.0, |kib| kib / (1u64 << 20) as f64)
    }
}

/// A short command's standard output, trimmed; empty when it isn't installed, fails or takes
/// longer than 20 seconds.
pub(crate) fn run_quiet(program: &str, args: &[&str]) -> String {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::run::hide_window(&mut cmd);
    let Ok(mut child) = cmd.spawn() else {
        return String::new();
    };
    let mut stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_string(&mut out);
        }
        out
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    reader.join().unwrap_or_default().trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(os: Os, arch: Arch, ram_gb: f64, gpu: Gpu) -> Machine {
        Machine {
            os,
            arch,
            ram_gb,
            gpu,
            gpu_name: "GPU".into(),
            vram_gb: 4.0,
        }
    }

    #[test]
    fn the_backend_follows_the_platform_and_the_gpu() {
        let pick = |os, arch, gpu| pick_backend(&machine(os, arch, 32.0, gpu));
        assert_eq!(pick(Os::Macos, Arch::Arm64, Gpu::Apple), Backend::Mlx);
        assert_eq!(pick(Os::Macos, Arch::X86_64, Gpu::None), Backend::Cpu);
        assert_eq!(pick(Os::Windows, Arch::X86_64, Gpu::Nvidia), Backend::Cuda);
        assert_eq!(pick(Os::Linux, Arch::X86_64, Gpu::Nvidia), Backend::Vulkan);
        assert_eq!(pick(Os::Windows, Arch::X86_64, Gpu::Other), Backend::Vulkan);
        assert_eq!(pick(Os::Linux, Arch::Arm64, Gpu::None), Backend::Cpu);
    }

    #[test]
    fn ram_decides_the_tier() {
        let tier = |ram| pick_tier(&machine(Os::Windows, Arch::X86_64, ram, Gpu::Nvidia));
        assert_eq!(tier(32.0), Tier::Q8);
        assert_eq!(tier(24.0), Tier::Q8);
        assert_eq!(tier(16.0), Tier::Q4);
    }

    #[test]
    fn ids_round_trip() {
        for b in Backend::ALL {
            assert_eq!(Backend::parse(b.id()), Some(b));
        }
        assert_eq!(Backend::parse("auto"), None);
        assert_eq!(Tier::parse("q4"), Some(Tier::Q4));
        assert_eq!(Tier::parse("q5"), None);
    }

    #[test]
    fn basic_and_virtual_adapters_dont_count() {
        let listing =
            "Microsoft Basic Display Adapter\r\n  AMD Radeon(TM) Graphics \r\nVirtual Display\n";
        assert_eq!(real_adapters(listing), ["AMD Radeon(TM) Graphics"]);
    }

    #[test]
    fn a_machine_describes_itself_in_one_line() {
        let mut m = machine(Os::Windows, Arch::X86_64, 31.7, Gpu::Nvidia);
        m.gpu_name = "NVIDIA GeForce RTX 3050 Ti Laptop GPU".into();
        assert_eq!(
            m.describe(),
            "windows x86_64, 32 GB RAM, NVIDIA GeForce RTX 3050 Ti Laptop GPU (4 GB)"
        );
        m.gpu = Gpu::None;
        assert!(m.describe().ends_with("no GPU found"));
    }

    #[test]
    fn this_computer_has_memory() {
        assert!(ram_gb() > 0.5, "{}", ram_gb());
        assert_eq!(run_quiet("folderskin-no-such-program", &[]), "");
    }
}
