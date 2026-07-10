use nvml_wrapper::Nvml;
fn main() {
    if let Ok(nvml) = Nvml::init() {
        if let Ok(device) = nvml.device_by_index(0) {
            if let Ok(procs) = device.running_compute_processes() {
                for p in procs {
                    match p.used_gpu_memory {
                        nvml_wrapper::enum_wrappers::device::UsedGpuMemory::Used(bytes) => {
                            println!("pid {}: {} bytes", p.pid, bytes);
                        }
                        nvml_wrapper::enum_wrappers::device::UsedGpuMemory::Unavailable => {
                            println!("pid {}: unavailable", p.pid);
                        }
                    }
                }
            }
        }
    }
}
