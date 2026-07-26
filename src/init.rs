use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use anyhow::{anyhow, bail, Context, Result};
use bytesize::ByteSize;
use sysinfo::Disks;

use crate::collectors::disk::device_short_name;
use crate::config::{self, Config, Tab, INTERVALS};
use crate::ui::theme::ThemeMode;

const DEFAULT_BENCHMARK_MB: u64 = 256;
const MIN_BENCHMARK_MB: u64 = 64;
const MAX_BENCHMARK_MB: u64 = 4096;
const BENCHMARK_BLOCK_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone)]
struct DiskChoice {
    device_short: String,
    mount_point: PathBuf,
    available_bytes: u64,
    writable: bool,
}

pub fn run() -> Result<()> {
    let mut cfg = config::load();
    println!("\nrtop init — configuración interactiva\n");
    println!(
        "El benchmark crea un archivo temporal, lo sincroniza a disco y lo elimina al terminar."
    );
    println!("No se ejecutará ninguna prueba de escritura sin tu confirmación.\n");

    let disks = mounted_disks();
    let selected_disk = select_disk(&disks, cfg.selected_disk.as_deref())?;
    if let Some(disk) = &selected_disk {
        cfg.selected_disk = Some(disk.device_short.clone());
    }

    if let Some(disk) = selected_disk {
        if !disk.writable {
            println!(
                "{} es de solo lectura; no se puede ejecutar un benchmark allí. Selecciona un punto de montaje grabable.",
                disk.mount_point.display()
            );
        } else if ask_yes_no(
            "¿Quieres medir la capacidad de escritura de este disco?",
            false,
        )? {
            let size_mb = prompt_benchmark_size()?;
            if size_mb.saturating_mul(1_000_000) > disk.available_bytes.saturating_sub(64_000_000) {
                bail!(
                    "No hay espacio libre suficiente en {} para una prueba de {} MB.",
                    disk.mount_point.display(),
                    size_mb
                );
            }

            let benchmark_dir = resolve_benchmark_dir(&disk.mount_point)?;
            println!(
                "\nLa prueba escribirá y eliminará {} en {}.",
                ByteSize(size_mb * 1_000_000),
                benchmark_dir.display()
            );
            if ask_yes_no("¿Confirmas que deseas iniciar la prueba?", false)? {
                let measured_mb_s = benchmark_write_mb_s(&benchmark_dir, size_mb)?;
                let detected_capacity = measured_mb_s.round().max(1.0) as u64;
                println!("Resultado: {:.1} MB/s", measured_mb_s);
                if ask_yes_no(
                    &format!(
                        "¿Guardar {} MB/s como capacidad de E/S de disco?",
                        detected_capacity
                    ),
                    true,
                )? {
                    cfg.disk_io_capacity_mb_s = detected_capacity;
                }
            }
        }
    } else {
        println!("No se detectaron discos montados; se omitió el benchmark.");
    }

    configure_additional_options(&mut cfg)?;
    config::save(&cfg)?;
    println!(
        "\nConfiguración guardada en {}",
        config::config_path().display()
    );
    Ok(())
}

fn mounted_disks() -> Vec<DiskChoice> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| !disk.mount_point().as_os_str().is_empty())
        .map(|disk| DiskChoice {
            device_short: device_short_name(&disk.name().to_string_lossy()),
            mount_point: disk.mount_point().to_path_buf(),
            available_bytes: disk.available_space(),
            writable: is_writable_mount(disk.mount_point()),
        })
        .collect()
}

#[cfg(unix)]
fn is_writable_mount(path: &Path) -> bool {
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path` is NUL-terminated and `stats` points to valid writable storage.
    let result = unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return false;
    }
    // SAFETY: a successful `statvfs` call initializes the supplied structure.
    let stats = unsafe { stats.assume_init() };
    stats.f_flag & libc::ST_RDONLY == 0
}

#[cfg(not(unix))]
fn is_writable_mount(_path: &Path) -> bool {
    true
}

#[cfg(unix)]
fn is_writable_path(path: &Path) -> bool {
    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `c_path` is NUL-terminated and `access` only reads it.
    unsafe { libc::access(c_path.as_ptr(), libc::W_OK) == 0 }
}

#[cfg(not(unix))]
fn is_writable_path(_path: &Path) -> bool {
    true
}

#[cfg(unix)]
fn same_device(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(meta_a), Ok(meta_b)) => meta_a.dev() == meta_b.dev(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn same_device(_a: &Path, _b: &Path) -> bool {
    true
}

/// El punto de montaje raíz puede pertenecer a root (p. ej. `/System/Volumes/Data`
/// en macOS) aunque el filesystem admita escritura. En ese caso, busca un
/// subdirectorio del mismo volumen donde el usuario actual sí pueda escribir.
fn resolve_benchmark_dir(mount_point: &Path) -> Result<PathBuf> {
    if is_writable_path(mount_point) {
        return Ok(mount_point.to_path_buf());
    }

    let candidates = [dirs::home_dir(), Some(std::env::temp_dir())];
    for candidate in candidates.into_iter().flatten() {
        if is_writable_path(&candidate) && same_device(&candidate, mount_point) {
            return Ok(candidate);
        }
    }

    bail!(
        "No se encontró un directorio con permiso de escritura dentro de {}",
        mount_point.display()
    );
}

fn select_disk(disks: &[DiskChoice], current: Option<&str>) -> Result<Option<DiskChoice>> {
    if disks.is_empty() {
        return Ok(None);
    }

    println!("Discos montados detectados:");
    for (idx, disk) in disks.iter().enumerate() {
        println!(
            "  {}) {} en {} (libre: {}; {})",
            idx + 1,
            disk.device_short,
            disk.mount_point.display(),
            ByteSize(disk.available_bytes),
            if disk.writable {
                "grabable"
            } else {
                "solo lectura"
            }
        );
    }

    let default_idx = current
        .and_then(|selected| {
            disks
                .iter()
                .position(|disk| disk.device_short == selected && disk.writable)
        })
        .or_else(|| disks.iter().position(|disk| disk.writable))
        .unwrap_or(0);
    loop {
        let input = prompt(&format!(
            "Selecciona el disco a monitorear [{}]: ",
            default_idx + 1
        ))?;
        if input.is_empty() {
            return Ok(Some(disks[default_idx].clone()));
        }
        if let Ok(idx) = input.parse::<usize>() {
            if let Some(disk) = disks.get(idx.saturating_sub(1)) {
                return Ok(Some(disk.clone()));
            }
        }
        println!("Selecciona un número entre 1 y {}.", disks.len());
    }
}

fn prompt_benchmark_size() -> Result<u64> {
    loop {
        let input = prompt(&format!(
            "Tamaño de la prueba en MB ({}–{}, por defecto {}): ",
            MIN_BENCHMARK_MB, MAX_BENCHMARK_MB, DEFAULT_BENCHMARK_MB
        ))?;
        if input.is_empty() {
            return Ok(DEFAULT_BENCHMARK_MB);
        }
        if let Ok(size_mb) = input.parse::<u64>() {
            if (MIN_BENCHMARK_MB..=MAX_BENCHMARK_MB).contains(&size_mb) {
                return Ok(size_mb);
            }
        }
        println!("Ingresa un valor entero entre {MIN_BENCHMARK_MB} y {MAX_BENCHMARK_MB}.");
    }
}

fn configure_additional_options(cfg: &mut Config) -> Result<()> {
    if !ask_yes_no("¿Quieres configurar más opciones?", false)? {
        return Ok(());
    }

    loop {
        println!(
            "\nOpciones adicionales:\n  1) Intervalo de actualización\n  2) Interfaz de red\n  3) Pestaña inicial\n  4) Mostrar Swap\n  5) Socket de Docker\n  6) Tema\n  7) Capacidad de E/S de disco manual\n  0) Guardar y finalizar"
        );
        match prompt("Elige una opción [0]: ")?.as_str() {
            "" | "0" => return Ok(()),
            "1" => configure_refresh_interval(cfg)?,
            "2" => configure_network_interface(cfg)?,
            "3" => configure_default_tab(cfg)?,
            "4" => cfg.show_swap = ask_yes_no("¿Mostrar Swap?", cfg.show_swap)?,
            "5" => configure_docker_socket(cfg)?,
            "6" => configure_theme(cfg)?,
            "7" => configure_disk_capacity(cfg)?,
            _ => println!("Opción no válida."),
        }
    }
}

fn configure_refresh_interval(cfg: &mut Config) -> Result<()> {
    loop {
        let input = prompt(&format!(
            "Intervalo en segundos {:?} (actual {}): ",
            INTERVALS, cfg.refresh_interval_secs
        ))?;
        if input.is_empty() {
            return Ok(());
        }
        if let Ok(interval) = input.parse::<f64>() {
            if INTERVALS
                .iter()
                .any(|&value| (value - interval).abs() < f64::EPSILON)
            {
                cfg.refresh_interval_secs = interval;
                return Ok(());
            }
        }
        println!("Usa uno de los valores mostrados.");
    }
}

fn configure_network_interface(cfg: &mut Config) -> Result<()> {
    let current = cfg.selected_nic.as_deref().unwrap_or("todas");
    let input = prompt(&format!(
        "Interfaz de red (actual {current}; Enter conserva, '-' usa todas): "
    ))?;
    if input == "-" {
        cfg.selected_nic = None;
    } else if !input.is_empty() {
        cfg.selected_nic = Some(input);
    }
    Ok(())
}

fn configure_default_tab(cfg: &mut Config) -> Result<()> {
    loop {
        let input = prompt("Pestaña inicial: procesos, contenedores o red (Enter conserva): ")?;
        match input.to_lowercase().as_str() {
            "" => return Ok(()),
            "procesos" | "processes" => {
                cfg.default_tab = Tab::Processes;
                return Ok(());
            }
            "contenedores" | "containers" => {
                cfg.default_tab = Tab::Containers;
                return Ok(());
            }
            "red" | "network" => {
                cfg.default_tab = Tab::Network;
                return Ok(());
            }
            _ => println!("Escribe procesos, contenedores o red."),
        }
    }
}

fn configure_docker_socket(cfg: &mut Config) -> Result<()> {
    let current = cfg
        .docker_socket_path
        .as_deref()
        .unwrap_or("predeterminado");
    let input = prompt(&format!(
        "Ruta al socket Docker (actual {current}; Enter conserva, '-' elimina): "
    ))?;
    if input == "-" {
        cfg.docker_socket_path = None;
    } else if !input.is_empty() {
        cfg.docker_socket_path = Some(input);
    }
    Ok(())
}

fn configure_theme(cfg: &mut Config) -> Result<()> {
    loop {
        let input = prompt(&format!(
            "Tema: dark, light, nord, matrix, sunset, dracula, gruvbox, tokyo_night (actual {}; Enter conserva): ",
            cfg.theme.name()
        ))?;
        let theme = match input.to_lowercase().as_str() {
            "" => return Ok(()),
            "dark" => ThemeMode::Dark,
            "light" => ThemeMode::Light,
            "nord" => ThemeMode::Nord,
            "matrix" => ThemeMode::Matrix,
            "sunset" => ThemeMode::Sunset,
            "dracula" => ThemeMode::Dracula,
            "gruvbox" => ThemeMode::Gruvbox,
            "tokyo_night" | "tokyo night" => ThemeMode::TokyoNight,
            _ => {
                println!("Tema no válido.");
                continue;
            }
        };
        cfg.theme = theme;
        return Ok(());
    }
}

fn configure_disk_capacity(cfg: &mut Config) -> Result<()> {
    loop {
        let input = prompt(&format!(
            "Capacidad de E/S en MB/s (actual {}): ",
            cfg.disk_io_capacity_mb_s
        ))?;
        if input.is_empty() {
            return Ok(());
        }
        if let Ok(capacity) = input.parse::<u64>() {
            if capacity > 0 {
                cfg.disk_io_capacity_mb_s = capacity;
                return Ok(());
            }
        }
        println!("Ingresa un entero mayor que cero.");
    }
}

fn ask_yes_no(question: &str, default: bool) -> Result<bool> {
    let hint = if default { "S/n" } else { "s/N" };
    loop {
        match prompt(&format!("{question} [{hint}]: "))?
            .to_lowercase()
            .as_str()
        {
            "" => return Ok(default),
            "s" | "si" | "sí" | "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Responde s o n."),
        }
    }
}

fn prompt(message: &str) -> Result<String> {
    print!("{message}");
    io::stdout().flush()?;
    let mut input = String::new();
    if io::stdin().read_line(&mut input)? == 0 {
        bail!("Entrada cerrada; no se modificó la configuración.");
    }
    Ok(input.trim().to_string())
}

fn benchmark_write_mb_s(write_dir: &Path, size_mb: u64) -> Result<f64> {
    let bytes_to_write = size_mb
        .checked_mul(1_000_000)
        .ok_or_else(|| anyhow!("Tamaño de benchmark demasiado grande"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = write_dir.join(format!(
        ".rtop-io-benchmark-{}-{nonce}.tmp",
        std::process::id()
    ));
    let mut block = vec![0_u8; BENCHMARK_BLOCK_BYTES];
    for (index, byte) in block.iter_mut().enumerate() {
        *byte = (index as u8).wrapping_mul(31).wrapping_add(17);
    }

    let result = (|| -> Result<f64> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| {
                format!(
                    "No se pudo crear el archivo temporal en {}",
                    write_dir.display()
                )
            })?;
        let started = Instant::now();
        let mut remaining = bytes_to_write;
        while remaining > 0 {
            let write_len = remaining.min(block.len() as u64) as usize;
            file.write_all(&block[..write_len])?;
            remaining -= write_len as u64;
        }
        file.sync_all()
            .context("No se pudo sincronizar el benchmark con el disco")?;
        let elapsed = started.elapsed().as_secs_f64();
        if elapsed <= 0.0 {
            bail!("La duración del benchmark no es válida");
        }
        Ok(bytes_to_write as f64 / elapsed / 1_000_000.0)
    })();

    let cleanup_result = fs::remove_file(&path);
    let speed = result?;
    cleanup_result.context("La prueba terminó, pero no se pudo eliminar su archivo temporal")?;
    Ok(speed)
}

#[cfg(test)]
mod tests {
    use super::{benchmark_write_mb_s, is_writable_path, resolve_benchmark_dir};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn benchmark_creates_and_removes_its_temporary_file() {
        let dir = std::env::temp_dir();
        let speed = benchmark_write_mb_s(&dir, 1).expect("benchmark should finish");
        assert!(speed.is_finite() && speed > 0.0);
    }

    #[cfg(unix)]
    fn running_as_root() -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(unix)]
    fn make_unwritable_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
        dir
    }

    #[cfg(unix)]
    fn cleanup_unwritable_dir(dir: &std::path::Path) {
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    #[cfg(unix)]
    fn is_writable_path_reports_false_for_read_only_directory() {
        if running_as_root() {
            return;
        }
        let dir = make_unwritable_dir("rtop-init-test-ro-detect");
        let writable = is_writable_path(&dir);
        cleanup_unwritable_dir(&dir);
        assert!(!writable);
    }

    #[test]
    #[cfg(unix)]
    fn resolve_benchmark_dir_falls_back_to_writable_directory_on_same_device() {
        // Reproduces the reported bug: a mount point whose root is owned by
        // another user (e.g. /System/Volumes/Data on macOS) is filesystem-writable
        // but not writable by the current user. The resolver must not return that
        // root; it must find a writable directory on the same device instead.
        if running_as_root() {
            return;
        }
        let dir = make_unwritable_dir("rtop-init-test-ro-fallback");
        let resolved = resolve_benchmark_dir(&dir);
        cleanup_unwritable_dir(&dir);

        let resolved = resolved.expect("should fall back instead of failing");
        assert_ne!(resolved, dir);
        assert!(is_writable_path(&resolved));
    }
}
