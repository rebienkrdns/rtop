# rtop 🚀

[![CI](https://github.com/usuario/rtop/actions/workflows/ci.yml/badge.svg)](https://github.com/usuario/rtop/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/rtop.svg)](https://crates.io/crates/rtop)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#licencia)

`rtop` es un monitor de recursos del sistema moderno, rápido y ligero escrito en Rust para la terminal (TUI). A diferencia de otros monitores clásicos, `rtop` destaca por su integración nativa con Docker/Podman y su monitorización detallada de lectura/escritura de disco (I/O) a nivel de sistema y de procesos individuales.

---

## 📸 Demostración

*(Graba tu terminal usando `asciinema` y añade el reproductor o un GIF animado aquí)*

```
  rtop                srv-prod · Ubuntu 22.04    Refresco: [ ◀ 2s ▶ ]    14:32:01
  ─────────────────────────────────────────────────────────────────────────────────

  CPU                                                                         45%
  ████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░

  Memoria                                                      5.1 GB / 8.0 GB  62%
  ████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░

  Disco  /dev/nvme0n1p1 (/)                                                  78%
  ██████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
  ↑ Escritura  1.1 MB/s          ↓ Lectura  4.2 MB/s            [ F2 cambiar ]

  Red  eth0                         ↓ Entrada  8.1 MB/s    ↑ Salida  2.3 MB/s
                                                                   [ F3 cambiar ]

  ─────────────────────────────────────────────────────────────────────────────────
  [ Procesos ]  [ Contenedores ]
  ─────────────────────────────────────────────────────────────────────────────────
  Nombre           CPU%    RAM          Disco R      Disco W      Estado
  ─────────────────────────────────────────────────────────────────────────────────
  postgres         2.3%   312 MB       4.2 MB/s     1.1 MB/s     ● ejecutando
  node             0.8%   128 MB         0 B/s       2.0 MB/s    ● ejecutando
  python           0.2%    64 MB         0 B/s         0 B/s     ● ejecutando
  nginx            0.1%    45 MB         0 B/s         0 B/s     ● ejecutando
  ─────────────────────────────────────────────────────────────────────────────────
  F1 Ayuda   F2 Disco   F3 Red   Tab Pestañas   / Filtrar   Enter Detalle   Q Salir
```

---

## ⚡ Instalación

### 1. Script de instalación rápida (Linux y macOS)
Detecta automáticamente tu sistema operativo y arquitectura, descarga el binario precompilado más reciente y lo instala en `/usr/local/bin/`:

```bash
curl -fsSL https://github.com/usuario/rtop/raw/master/install.sh | sh
```

### 2. Desde Crates.io (Recomendado para usuarios de Rust)
Si tienes Rust y Cargo instalados en tu sistema:

```bash
cargo install rtop
```

### 3. macOS (Homebrew)
Puedes instalar `rtop` agregando nuestro tap personalizado:

```bash
brew tap usuario/rtop
brew install rtop
```

### 4. Paquetes para distribuciones Linux
Descarga los paquetes desde la sección de [Releases de GitHub](https://github.com/usuario/rtop/releases):
*   **Debian/Ubuntu (`.deb`):** `sudo dpkg -i rtop_*.deb`
*   **RHEL/CentOS/Fedora (`.rpm`):** `sudo rpm -i rtop_*.rpm`

### 5. Compilar desde la fuente
```bash
git clone https://github.com/usuario/rtop.git
cd rtop
cargo build --release
```
El binario optimizado estará en `target/release/rtop`.

---

## 📊 Diferencias con alternativas

| Característica | `rtop` 🚀 | `btop` | `ctop` | `htop` |
| :--- | :---: | :---: | :---: | :---: |
| **Escrito en** | **Rust** | C++ | Go | C |
| **Uso de memoria** | **Mínimo (< 15MB)** | Bajo | Medio | Mínimo |
| **Monitoreo Docker/Podman** | **Sí (Nativo)** | No | Sí (Solo containers) | No |
| **Escritura/Lectura de Disco por Proceso** | **Sí (Lectura directa)** | No | No | No |
| **Fácil Empaquetado** | **Sí (`cargo`)** | Sí | Sí | Sí |

---

## ⚙️ Configuración

`rtop` almacena su archivo de configuración en `~/.config/rtop/config.toml` (o en la ruta especificada por la variable de entorno `RTOP_CONFIG_PATH`). El archivo se genera automáticamente con los valores predeterminados la primera vez que se ejecuta.

### Ejemplo de `config.toml`

```toml
# Intervalo de actualización en segundos (valores soportados: 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0)
refresh_interval_secs = 2.0

# Dispositivo de disco predeterminado para el monitoreo de I/O (ej. "nvme0n1", "sda")
# Si se deja como None, rtop intentará autodetectar el disco principal
selected_disk = "nvme0n1"

# Interfaz de red predeterminada a monitorear (ej. "eth0", "wlan0")
# Si es None, se muestra la suma de tráfico de todas las interfaces activas
selected_nic = "eth0"

# Pestaña activa al iniciar la aplicación ("processes" o "containers")
default_tab = "processes"

# Columna por la cual ordenar la lista de procesos ("cpu", "memory", "pid", "name")
process_sort_column = "cpu"

# Mostrar u ocultar la sección de memoria Swap
show_swap = true

# Ruta personalizada al socket del motor de Docker (ej. "/var/run/docker.sock")
# docker_socket_path = "/var/run/docker.sock"
```

---

## ⌨️ Atajos de Teclado

| Tecla | Acción |
| :---: | :--- |
| `q` / `Ctrl+C` | Salir de la aplicación |
| `Tab` | Cambiar de pestaña (Procesos ↔ Contenedores) |
| `↑` / `↓` | Navegar por la lista |
| `Enter` | Ver detalle del proceso o contenedor seleccionado |
| `ESC` | Volver a la pantalla anterior / Cerrar menú de ayuda o selector |
| `F1` | Mostrar pantalla de ayuda y atajos de teclado |
| `F2` | Abrir selector de disco para monitoreo de I/O |
| `F3` | Abrir selector de interfaz de red |
| `[` | Reducir el intervalo de actualización (más rápido) |
| `]` | Aumentar el intervalo de actualización (más lento) |
| `c` | Ordenar procesos por uso de CPU |
| `m` | Ordenar procesos por uso de Memoria RAM |
| `r` | Ordenar procesos por Lectura de Disco (Disk Read) |
| `w` | Ordenar procesos por Escritura de Disco (Disk Write) |
| `/` | Filtrar elementos de la lista por nombre |
| `L` | *(Contenedores)* Ver logs en la pantalla de detalle |
| `R` | *(Contenedores)* Reiniciar contenedor (solicita confirmación) |
| `S` | *(Contenedores)* Detener contenedor (solicita confirmación) |

---

## 🛠️ Solución de Problemas comunes

### 1. Permisos del Socket de Docker
Si al ir a la pestaña `Contenedores` obtienes un error de conexión, es probable que tu usuario actual no tenga permisos para leer el socket de Docker.
*   **Solución:** Añade tu usuario al grupo `docker`:
    ```bash
    sudo usermod -aG docker $USER
    ```
    *(Recuerda cerrar sesión y volver a iniciarla para aplicar los cambios).*
*   Si estás usando Podman o un path de socket no estándar, configúralo en `config.toml` con la opción `docker_socket_path`.

### 2. Permisos del sistema de archivos `/proc`
En ciertos contenedores o entornos Docker muy restringidos, `rtop` podría no tener acceso de lectura a `/proc` para obtener métricas del sistema.
*   **Solución:** Asegúrate de que el contenedor se ejecute con acceso adecuado, por ejemplo compartiendo el `/proc` del host si estás monitoreando el host desde un contenedor:
    ```bash
    docker run --privileged -v /proc:/host/proc:ro rtop
    ```

---

## 📄 Licencia

Este proyecto está licenciado bajo la Licencia **MIT** o **Apache-2.0** (Licencia Dual), lo que garantiza su libre uso, modificación y distribución comercial o privada.
