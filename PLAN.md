# rtop — Plan de Ejecución del Proyecto

> Monitor de sistema unificado escrito en Rust: procesos, contenedores Docker/Podman, disco con I/O en tiempo real, red con selector de interfaz, e intervalos de refresco configurables. Inspirado en btop, pero más simple, más intuitivo y más completo.

---

## Índice

1. [Visión general del proyecto](#1-visión-general-del-proyecto)
2. [Filosofía de diseño y UX](#2-filosofía-de-diseño-y-ux)
3. [Arquitectura técnica](#3-arquitectura-técnica)
4. [Stack tecnológico](#4-stack-tecnológico)
5. [Estructura del proyecto](#5-estructura-del-proyecto)
6. [Fase 1 — Scaffold base](#6-fase-1--scaffold-base)
7. [Fase 2 — Métricas del sistema](#7-fase-2--métricas-del-sistema)
8. [Fase 3 — Disco avanzado con I/O en tiempo real](#8-fase-3--disco-avanzado-con-io-en-tiempo-real)
9. [Fase 4 — Red con selector de interfaz](#9-fase-4--red-con-selector-de-interfaz)
10. [Fase 5 — Procesos con I/O por proceso](#10-fase-5--procesos-con-io-por-proceso)
11. [Fase 6 — Contenedores Docker y Podman](#11-fase-6--contenedores-docker-y-podman)
12. [Fase 7 — Vistas de detalle](#12-fase-7--vistas-de-detalle)
13. [Fase 8 — Intervalo de refresco configurable](#13-fase-8--intervalo-de-refresco-configurable)
14. [Fase 9 — Configuración persistente](#14-fase-9--configuración-persistente)
15. [Fase 10 — Polish de UX y accesibilidad visual](#15-fase-10--polish-de-ux-y-accesibilidad-visual)
16. [Fase 11 — Empaquetado y release](#16-fase-11--empaquetado-y-release)
17. [Referencia de atajos de teclado](#17-referencia-de-atajos-de-teclado)
18. [Diseño visual del layout](#18-diseño-visual-del-layout)
19. [Fuentes de datos técnicas](#19-fuentes-de-datos-técnicas)
20. [Criterios de éxito por fase](#20-criterios-de-éxito-por-fase)

---

## 1. Visión general del proyecto

### ¿Qué es rtop?

`rtop` es un monitor de sistema de terminal (TUI) escrito completamente en Rust que unifica en una sola herramienta lo que hoy requiere usar múltiples programas simultáneamente: `btop`, `ctop`, `iotop`, `nethogs`, entre otros.

### Problema que resuelve

| Problema actual | Solución en rtop |
|----------------|-----------------|
| btop no muestra correctamente discos EBS en AWS | Selector manual de dispositivo de disco |
| btop no muestra I/O de disco por proceso | Lectura de `/proc/{PID}/io` por proceso |
| ctop no muestra I/O de disco por contenedor | Lectura de cgroups v2 por contenedor |
| Para monitorear contenedores hay que abrir ctop aparte | Pestaña de contenedores integrada |
| btop muestra todas las NICs mezcladas | Selector de interfaz de red con tecla rápida |
| No hay herramienta que unifique todo esto | rtop lo hace todo desde una sola terminal |

### Público objetivo

- Administradores de servidores Linux y AWS/cloud
- Desarrolladores que trabajan con Docker o Podman
- Equipos de DevOps y SRE
- Cualquier persona que quiera entender qué está haciendo su sistema de forma visual e intuitiva

### Plataformas objetivo

- **Linux** (objetivo primario — donde vive la mayoría de los servidores)
- **macOS** (objetivo secundario — útil para desarrolladores)
- Windows: fuera del alcance inicial

---

## 2. Filosofía de diseño y UX

### Principio central: "Lo ve cualquiera, lo entiende cualquiera"

rtop debe poder ser usado por alguien que nunca ha visto `htop` y al mismo tiempo ser suficientemente potente para un administrador de sistemas senior. La información debe hablar por sí misma.

### Reglas de diseño

**1. Colores semánticos — siempre consistentes**

| Color | Significado | Dónde aplica |
|-------|------------|--------------|
| Verde | Sano, normal | Uso < 60% |
| Amarillo | Atención | Uso 60–85% |
| Rojo | Crítico | Uso > 85% |
| Cian | Títulos y etiquetas | Headers de sección |
| Blanco | Datos normales | Valores numéricos |
| Gris | Metadata secundaria | Subtítulos, unidades |

**2. Menos bordes, más espacio**

btop usa `│`, `─`, `┌`, `┐` agresivamente. rtop usará líneas separadoras simples (`─`) y espaciado generoso para que la vista respire.

**3. Labels en lenguaje humano**

- En vez de `nvme0n1p1` → mostrar `Disco principal · /dev/nvme0n1p1 (/)` 
- En vez de `ens5` → mostrar `Red · ens5 (principal)`
- En vez de un PID numérico seco → mostrar nombre del proceso en primer lugar

**4. Footer siempre visible con atajos**

Como `nano`, siempre en la parte inferior de la pantalla:
```
F1 Ayuda  F2 Disco  F3 Red  Tab Pestañas  Enter Detalle  Q Salir
```

**5. Feedback inmediato**

Cada acción del usuario (cambiar intervalo, seleccionar disco, filtrar) debe reflejarse visualmente en menos de 100ms, aunque los datos tarden más en actualizarse.

**6. Gráficas de barras como btop**

Las barras de progreso son la forma más intuitiva de mostrar porcentajes. Se mantienen, pero con colores semánticos consistentes y sin decoración excesiva.

---

## 3. Arquitectura técnica

### Modelo de capas

```
┌─────────────────────────────────────────────────┐
│                   UI Layer                       │
│   ratatui widgets · layout · colores · input     │
├─────────────────────────────────────────────────┤
│                  App State                       │
│   Estado global · configuración · selecciones   │
├──────────────────┬──────────────────────────────┤
│  System Collector│  Container Collector          │
│  sysinfo crate   │  bollard (Docker API socket)  │
│  /proc/{pid}/io  │  cgroups v2 (disk I/O)        │
│  /sys/block/*/   │  Podman socket compat         │
├──────────────────┴──────────────────────────────┤
│               Async Runtime (tokio)              │
│   Polling periódico · canales mpsc · intervals   │
└─────────────────────────────────────────────────┘
```

### Modelo de datos y flujo

```
tokio::spawn → collector loop (cada N segundos)
      │
      ▼
mpsc::channel → envía snapshot de datos
      │
      ▼
App State se actualiza (Mutex<AppData>)
      │
      ▼
ratatui render loop (cada frame) lee el estado
      │
      ▼
Terminal se redibuja
```

### Manejo del estado

El estado global (`AppState`) contendrá:

```rust
struct AppState {
    // Métricas del sistema
    cpu: CpuData,
    memory: MemoryData,
    disk: DiskData,
    network: NetworkData,
    
    // Listas
    processes: Vec<ProcessData>,
    containers: Vec<ContainerData>,
    
    // Selecciones del usuario
    selected_disk: String,
    selected_nic: String,
    active_tab: Tab,
    selected_process_index: usize,
    selected_container_index: usize,
    
    // Configuración en tiempo de ejecución
    refresh_interval_secs: f64,
    
    // Vista actual
    view: View, // Main, ProcessDetail, ContainerDetail, DiskSelector, NicSelector
}
```

---

## 4. Stack tecnológico

### Dependencias principales (`Cargo.toml`)

```toml
[package]
name = "rtop"
version = "0.1.0"
edition = "2021"

[dependencies]
# TUI framework
ratatui = "0.26"
crossterm = "0.27"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Métricas del sistema
sysinfo = "0.30"

# Docker / Podman API
bollard = "0.16"

# Serialización (config file)
serde = { version = "1", features = ["derive"] }
toml = "0.8"

# Manejo de errores
anyhow = "1"
thiserror = "1"

# Formato de bytes/velocidades
bytesize = "1"

# Fechas y tiempos
chrono = "0.4"

# Logging (para debug)
tracing = "0.1"
tracing-subscriber = "0.3"
```

### Por qué cada crate

| Crate | Razón |
|-------|-------|
| `ratatui` | Sucesor activo de `tui-rs`. El más maduro para TUIs en Rust. Tiene layout flexbox-like, widgets de tabla, gráficas de barras, etc. |
| `crossterm` | Backend de terminal multiplataforma. Maneja input de teclado, colores ANSI, modo raw. |
| `tokio` | Runtime async estándar de facto en Rust. Permite polling concurrente de múltiples fuentes sin bloquear la UI. |
| `sysinfo` | Abstrae `/proc` en Linux y equivalentes en macOS. CPU, RAM, procesos, discos, red. Multiplataforma. |
| `bollard` | Cliente async nativo para la Docker Engine API via Unix socket. Soporta Docker y Podman (modo compat). |
| `serde` + `toml` | Para leer y escribir el archivo de configuración `~/.config/rtop/config.toml`. |
| `anyhow` | Manejo ergonómico de errores en código de aplicación. |
| `bytesize` | Formatea bytes como "1.2 MB", "450 KB", etc. automáticamente. |

---

## 5. Estructura del proyecto

```
rtop/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── .github/
│   └── workflows/
│       └── ci.yml               # CI: tests + clippy + fmt
├── src/
│   ├── main.rs                  # Entry point: init terminal, run app loop
│   ├── app.rs                   # AppState, lógica de eventos, coordinación
│   ├── config.rs                # Config struct, load/save desde TOML
│   ├── error.rs                 # Tipos de error personalizados
│   │
│   ├── collectors/              # Obtención de datos del sistema
│   │   ├── mod.rs
│   │   ├── system.rs            # CPU, RAM via sysinfo
│   │   ├── disk.rs              # Disco usado %, R/W en tiempo real
│   │   ├── network.rs           # NICs, ↑↓ en tiempo real
│   │   ├── processes.rs         # Lista de procesos + I/O por PID
│   │   └── containers.rs        # Docker/Podman via bollard + cgroups
│   │
│   ├── ui/                      # Renderizado con ratatui
│   │   ├── mod.rs               # Función principal draw()
│   │   ├── layout.rs            # Definición de áreas del layout
│   │   ├── theme.rs             # Colores, estilos, paleta semántica
│   │   ├── widgets/
│   │   │   ├── mod.rs
│   │   │   ├── cpu_bar.rs       # Widget barra de CPU
│   │   │   ├── memory_bar.rs    # Widget barra de RAM
│   │   │   ├── disk_bar.rs      # Widget disco + R/W rates
│   │   │   ├── network_bar.rs   # Widget red ↑↓
│   │   │   ├── process_table.rs # Tabla de procesos
│   │   │   ├── container_table.rs # Tabla de contenedores
│   │   │   └── footer.rs        # Footer con atajos de teclado
│   │   └── views/
│   │       ├── mod.rs
│   │       ├── main_view.rs     # Vista principal (dashboard)
│   │       ├── process_detail.rs # Detalle de un proceso
│   │       ├── container_detail.rs # Detalle de un contenedor
│   │       ├── disk_selector.rs # Modal para elegir disco
│   │       └── nic_selector.rs  # Modal para elegir interfaz de red
│   │
│   └── models/                  # Estructuras de datos
│       ├── mod.rs
│       ├── cpu.rs
│       ├── memory.rs
│       ├── disk.rs
│       ├── network.rs
│       ├── process.rs
│       └── container.rs
│
└── tests/
    ├── collectors_test.rs
    └── ui_test.rs
```

---

## 6. Fase 1 — Scaffold base

**Duración estimada:** 3–4 días  
**Objetivo:** Tener una aplicación Rust que abre el terminal en modo TUI, muestra algo en pantalla, y se puede cerrar con `q`. La base sobre la que se construye todo lo demás.

### Tareas

#### 1.1 Inicializar el proyecto

```bash
cargo new rtop
cd rtop
```

Agregar todas las dependencias al `Cargo.toml` según el stack definido en la Fase 4.

Verificar que compila limpio:
```bash
cargo build
cargo clippy
```

#### 1.2 Configurar el terminal en modo raw

En `src/main.rs`, implementar el patrón estándar de ratatui:

- Habilitar `raw mode` con crossterm (desactiva el echo y el buffering de línea)
- Crear el backend `CrosstermBackend`
- Crear el `Terminal` de ratatui
- Registrar un panic hook para restaurar el terminal si la app crashea (crucial — sin esto el terminal queda roto)
- Restaurar el terminal al salir limpiamente

```rust
// Patrón de inicialización
fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>>
fn restore_terminal(terminal: &mut Terminal<...>) -> anyhow::Result<()>
```

#### 1.3 Implementar el event loop principal

El loop principal en `src/app.rs` debe:

1. Llamar a `terminal.draw(|f| ui::draw(f, &state))` para renderizar
2. Llamar a `crossterm::event::poll()` con timeout para detectar input sin bloquear
3. Si hay input de teclado, procesarlo (por ahora, solo `q` para salir)
4. Repetir

Este loop corre en el thread principal. Los colectores corren en tasks de tokio aparte.

#### 1.4 Layout base vacío

En `src/ui/main_view.rs`, definir el layout inicial con ratatui usando `Layout::default()`:

```
┌─────────────────────────────────────────────┐
│  Header: nombre app + hostname + hora        │
├──────────────────┬──────────────────────────┤
│  Panel izquierdo │  Panel derecho            │
│  (CPU, RAM,      │  (Red)                    │
│   Disco)         │                           │
├──────────────────┴──────────────────────────┤
│  Panel inferior (Procesos / Contenedores)    │
├─────────────────────────────────────────────┤
│  Footer (atajos de teclado)                  │
└─────────────────────────────────────────────┘
```

Por ahora, cada panel muestra un bloque vacío con título. El objetivo es verificar que el layout se renderiza correctamente en distintos tamaños de terminal.

#### 1.5 Sistema de temas (colores)

En `src/ui/theme.rs`, definir la paleta de colores semántica:

```rust
pub struct Theme {
    pub ok: Color,      // verde
    pub warn: Color,    // amarillo
    pub crit: Color,    // rojo
    pub accent: Color,  // cian (títulos)
    pub text: Color,    // blanco
    pub muted: Color,   // gris
    pub bg: Color,      // negro o default terminal
}

impl Theme {
    pub fn color_for_pct(pct: f64) -> Color {
        if pct < 60.0 { Color::Green }
        else if pct < 85.0 { Color::Yellow }
        else { Color::Red }
    }
}
```

#### 1.6 Criterio de éxito de Fase 1

- `cargo run` abre la aplicación sin errores
- Se ve el layout con paneles vacíos y el footer
- Presionar `q` cierra la aplicación y restaura el terminal limpiamente
- No hay warnings de `cargo clippy`

---

## 7. Fase 2 — Métricas del sistema

**Duración estimada:** 3–4 días  
**Objetivo:** Mostrar CPU, RAM y disco (uso %) con barras de progreso en tiempo real.

### Tareas

#### 2.1 Modelo de datos del sistema

En `src/models/`, definir las estructuras:

```rust
// cpu.rs
pub struct CpuData {
    pub global_usage_pct: f64,
    pub per_core: Vec<f64>,
    pub core_count: usize,
}

// memory.rs
pub struct MemoryData {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub usage_pct: f64,
}

// disk.rs
pub struct DiskData {
    pub device: String,        // ej: "nvme0n1"
    pub mount_point: String,   // ej: "/"
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_pct: f64,
    // Fase 3 agrega R/W rates aquí
}
```

#### 2.2 Colector del sistema

En `src/collectors/system.rs`, implementar usando `sysinfo`:

```rust
use sysinfo::{System, SystemExt, CpuExt, DiskExt};

pub struct SystemCollector {
    sys: System,
}

impl SystemCollector {
    pub fn new() -> Self { ... }
    pub fn refresh(&mut self) { self.sys.refresh_all(); }
    pub fn cpu_data(&self) -> CpuData { ... }
    pub fn memory_data(&self) -> MemoryData { ... }
    pub fn disk_data(&self, device: &str) -> DiskData { ... }
}
```

El colector corre en una `tokio::task` que se despierta cada N segundos (controlado por el intervalo configurable), refresca los datos, y los envía al estado global via un `tokio::sync::mpsc::channel`.

#### 2.3 Widget de barra de CPU

En `src/ui/widgets/cpu_bar.rs`:

```
CPU                                            45%
████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░
```

- El color de la barra cambia según `Theme::color_for_pct()`
- El porcentaje se muestra alineado a la derecha
- El label "CPU" en cian

Implementar como función que recibe un `Frame`, un `Rect` de área, y un `&CpuData`.

#### 2.4 Widget de barra de Memoria

Similar a CPU. Mostrar también el valor absoluto:

```
Memoria                          5.1 GB / 8.0 GB  62%
████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░
```

#### 2.5 Widget de barra de Disco (uso %)

```
Disco  /dev/nvme0n1p1 (/)                      78%
██████████████████████████████░░░░░░░░░░░░░░░░░
```

- Mostrar el device y el mount point en el label
- El R/W en tiempo real se agrega en Fase 3

#### 2.6 Integrar en la vista principal

En `src/ui/views/main_view.rs`, integrar los tres widgets en el panel izquierdo del layout.

#### 2.7 Criterio de éxito de Fase 2

- Los tres widgets muestran datos reales del sistema
- Los colores cambian correctamente según el porcentaje
- Los datos se actualizan cada 2 segundos (valor por defecto)
- Si la terminal se redimensiona, el layout se adapta sin errores

---

## 8. Fase 3 — Disco avanzado con I/O en tiempo real

**Duración estimada:** 4–5 días  
**Objetivo:** Agregar lectura/escritura en tiempo real al widget de disco, implementar el selector manual de dispositivo (fix para AWS EBS), y mostrar I/O por proceso.

### El problema de AWS EBS

En instancias EC2, los volúmenes EBS aparecen como dispositivos `nvme` pero `sysinfo` puede no mapear correctamente el disco raíz vs volúmenes adicionales. La solución es leer directamente de `/proc/diskstats` y permitir al usuario elegir manualmente qué dispositivo monitorear.

### Tareas

#### 3.1 Leer I/O de disco del sistema desde `/proc/diskstats`

`/proc/diskstats` tiene este formato:
```
8  0  sda  1234  0  56789  200  4321  0  78901  300  0  500  500
```

Los campos relevantes son:
- Campo 3: nombre del dispositivo
- Campo 6: sectores leídos (acumulado)
- Campo 10: sectores escritos (acumulado)

Para obtener la **tasa** (bytes/segundo), se toman dos snapshots con diferencia de tiempo y se calcula el delta. Un sector = 512 bytes.

```rust
pub struct DiskIoSnapshot {
    pub timestamp: Instant,
    pub sectors_read: u64,
    pub sectors_written: u64,
}

pub struct DiskIoRate {
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

fn calculate_rate(prev: &DiskIoSnapshot, curr: &DiskIoSnapshot) -> DiskIoRate { ... }
```

#### 3.2 Actualizar el modelo de DiskData

```rust
pub struct DiskData {
    pub device: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_pct: f64,
    // Nuevo en Fase 3:
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}
```

#### 3.3 Actualizar el widget de disco

```
Disco  /dev/nvme0n1p1 (/)                      78%
██████████████████████████████░░░░░░░░░░░░░░░░░
↑ Escritura  1.1 MB/s     ↓ Lectura  4.2 MB/s
                                  [ F2 cambiar ]
```

- Las flechas `↑` (escritura) y `↓` (lectura) en colores distintos (naranja para escritura, azul para lectura)
- El hint `[ F2 cambiar ]` siempre visible

#### 3.4 Selector de disco (modal)

Cuando el usuario presiona `F2`, se muestra un modal que lista todos los dispositivos de bloque disponibles, leídos de `/proc/diskstats` o `/sys/block/`:

```
┌─── Seleccionar dispositivo de disco ──────────────┐
│                                                    │
│  > nvme0n1p1  /          237 GB   (seleccionado)  │
│    nvme1n1    /data       500 GB                   │
│    nvme2n1    (sin mount) 100 GB                   │
│                                                    │
│  ↑↓ navegar   Enter seleccionar   ESC cancelar     │
└────────────────────────────────────────────────────┘
```

Implementar como overlay usando `ratatui::widgets::Clear` + un bloque centrado sobre el layout principal.

La selección se guarda en `AppState.selected_disk` y también en el archivo de configuración.

#### 3.5 Leer I/O de disco por proceso desde `/proc/{PID}/io`

```
/proc/1842/io:
rchar: 2392842
wchar: 1773421
read_bytes: 409600    ← bytes leídos del disco físico
write_bytes: 122880   ← bytes escritos al disco físico
```

Igual que el disco del sistema, se necesitan dos snapshots para calcular la tasa.

```rust
pub struct ProcessIoData {
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}
```

**Nota importante:** Leer `/proc/{PID}/io` requiere permisos. Si `rtop` no corre como root, solo se puede leer el I/O de procesos del mismo usuario. En la UI, mostrar un indicador `(requiere sudo)` si el I/O es 0 para todos los procesos de otros usuarios.

#### 3.6 Criterio de éxito de Fase 3

- El widget de disco muestra lectura y escritura en tiempo real
- Presionar `F2` abre el modal de selección de disco
- Seleccionar un disco diferente actualiza todas las métricas inmediatamente
- La selección persiste entre reinicios
- En una instancia EC2 con múltiples volúmenes EBS, cada volumen aparece correctamente

---

## 9. Fase 4 — Red con selector de interfaz

**Duración estimada:** 2–3 días  
**Objetivo:** Mostrar el ancho de banda de red en tiempo real con un selector simple para elegir qué interfaz monitorear.

### Tareas

#### 4.1 Modelo de datos de red

```rust
pub struct NetworkData {
    pub interface: String,          // ej: "eth0", "ens5"
    pub recv_bytes_per_sec: f64,
    pub sent_bytes_per_sec: f64,
    pub total_recv_bytes: u64,      // acumulado desde inicio
    pub total_sent_bytes: u64,
}

pub struct NetworkInterface {
    pub name: String,
    pub is_up: bool,
    pub is_loopback: bool,
    pub ip_address: Option<String>,
}
```

#### 4.2 Colector de red

`sysinfo` provee métricas de red. Al igual que el disco, se necesitan dos snapshots para calcular la tasa.

La lógica de autodetección de la interfaz principal:
1. Filtrar interfaces loopback (`lo`)
2. Filtrar interfaces Docker (`docker0`, `br-*`, `veth*`)
3. Preferir la primera interfaz que esté `UP` con dirección IP asignada
4. Si hay varias, elegir la primera alfabéticamente
5. El usuario puede sobrescribir esta selección con `F3`

#### 4.3 Widget de red

```
Red  eth0  ↓ Entrada  8.1 MB/s     ↑ Salida  2.3 MB/s
                                          [ F3 cambiar ]
```

- `↓` en verde (datos que entran al servidor)
- `↑` en azul (datos que salen)
- Formato automático: B/s, KB/s, MB/s según la magnitud

#### 4.4 Selector de interfaz de red (modal)

Presionar `F3`:

```
┌─── Seleccionar interfaz de red ───────────────────┐
│                                                    │
│  > eth0      10.0.1.45    ● activa (seleccionada) │
│    docker0   172.17.0.1   ● activa                │
│    lo        127.0.0.1    ● loopback               │
│                                                    │
│  ↑↓ navegar   Enter seleccionar   ESC cancelar     │
└────────────────────────────────────────────────────┘
```

Las interfaces con estado `down` se muestran en gris y no se pueden seleccionar.

#### 4.5 Criterio de éxito de Fase 4

- El widget muestra entrada/salida en tiempo real con la interfaz autodetectada
- `F3` abre el selector correctamente
- Cambiar la interfaz actualiza las métricas inmediatamente
- En servidores con múltiples NICs (común en AWS), cada interfaz aparece correctamente

---

## 10. Fase 5 — Procesos con I/O por proceso

**Duración estimada:** 3–4 días  
**Objetivo:** Mostrar la tabla de procesos con CPU, RAM, y las columnas de I/O de disco que btop no tiene.

### Tareas

#### 5.1 Modelo de proceso

```rust
pub struct ProcessData {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub cpu_pct: f64,
    pub memory_bytes: u64,
    pub memory_pct: f64,
    pub disk_read_per_sec: f64,    // de /proc/{pid}/io
    pub disk_write_per_sec: f64,
    pub status: ProcessStatus,
    pub uptime_secs: u64,
    pub threads: u32,
}

pub enum ProcessStatus {
    Running,
    Sleeping,
    Stopped,
    Zombie,
}
```

#### 5.2 Colector de procesos

Combinar `sysinfo` (para CPU, RAM, nombre, usuario) con la lectura directa de `/proc/{PID}/io` (para I/O de disco).

El colector mantiene un `HashMap<u32, DiskIoSnapshot>` para calcular tasas por PID.

Cada ciclo de refresco:
1. Obtener lista de PIDs de `sysinfo`
2. Para cada PID, leer `/proc/{pid}/io` si es legible
3. Calcular delta con el snapshot anterior
4. Actualizar el HashMap con el nuevo snapshot

#### 5.3 Widget de tabla de procesos

Columnas:

| Nombre | CPU% | RAM | Disco R | Disco W | Estado |
|--------|------|-----|---------|---------|--------|

```
  Procesos                                          Filtrar: /
  ─────────────────────────────────────────────────────────────
  nginx         0.1%   45 MB    0 B/s    0 B/s   ● ejecutando
  postgres      2.3%  312 MB  4.2 MB/s  1.1 MB/s ● ejecutando
  python        0.8%  128 MB   0 B/s   2.0 MB/s  ● ejecutando
  node          0.2%   64 MB   0 B/s    0 B/s    ● ejecutando
  ─────────────────────────────────────────────────────────────
  123 procesos   ↑↓ navegar   Enter detalle   / filtrar
```

#### 5.4 Filtrado y búsqueda

Presionar `/` activa el modo de filtrado. El usuario escribe y la tabla se filtra en tiempo real por nombre de proceso. `ESC` cancela el filtro.

#### 5.5 Ordenamiento

Presionar las teclas de columna para ordenar:
- `c` → ordenar por CPU (default)
- `m` → ordenar por RAM
- `r` → ordenar por lectura de disco
- `w` → ordenar por escritura de disco

La columna de ordenamiento activo se muestra con `▼` o `▲`.

#### 5.6 Criterio de éxito de Fase 5

- La tabla muestra todos los procesos con CPU, RAM y I/O de disco
- Para procesos sin permiso de leer su I/O, mostrar `–` en lugar de `0`
- Filtrar por nombre funciona en tiempo real
- Ordenar por cualquier columna funciona
- Navegar con `↑↓` y seleccionar con `Enter` (detalle en Fase 7)

---

## 11. Fase 6 — Contenedores Docker y Podman

**Duración estimada:** 5–6 días  
**Objetivo:** Agregar una pestaña de contenedores con métricas completas: CPU, RAM, red, y el I/O de disco que ctop no tiene.

### Tareas

#### 6.1 Modelo de contenedor

```rust
pub struct ContainerData {
    pub id: String,            // primeros 12 chars del ID
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub uptime_secs: Option<u64>,
    pub cpu_pct: f64,
    pub memory_bytes: u64,
    pub memory_limit_bytes: u64,
    pub memory_pct: f64,
    pub net_recv_per_sec: f64,
    pub net_sent_per_sec: f64,
    pub disk_read_per_sec: f64,
    pub disk_write_per_sec: f64,
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
}

pub enum ContainerStatus {
    Running,
    Paused,
    Restarting,
    Exited,
    Dead,
}
```

#### 6.2 Conexión con Docker via bollard

```rust
use bollard::Docker;
use bollard::container::{ListContainersOptions, StatsOptions};

pub struct ContainerCollector {
    docker: Docker,
}

impl ContainerCollector {
    pub async fn new() -> anyhow::Result<Self> {
        // Intenta conectar via socket Unix: /var/run/docker.sock
        // Si falla, intenta el socket de Podman: /run/user/{uid}/podman/podman.sock
        let docker = Docker::connect_with_unix_defaults()?;
        Ok(Self { docker })
    }
}
```

Para obtener stats de CPU/RAM/red, usar el endpoint `GET /containers/{id}/stats?stream=false` de la Docker Engine API. bollard abstrae esto con `docker.stats(id, options)`.

#### 6.3 I/O de disco por contenedor via cgroups v2

Esta es la métrica que ctop no tiene. En Linux con cgroups v2 (kernels modernos, la mayoría de distribuciones actuales):

```
/sys/fs/cgroup/system.slice/docker-{CONTAINER_ID}.scope/io.stat
```

Formato:
```
8:0 rbytes=12345678 wbytes=87654321 rios=1234 wios=5678 dbytes=0 dios=0
```

- `rbytes`: bytes leídos del bloque
- `wbytes`: bytes escritos al bloque

Igual que con procesos, se necesitan dos snapshots para calcular la tasa.

**Fallback para cgroups v1** (kernels más viejos):
```
/sys/fs/cgroup/blkio/docker/{CONTAINER_ID}/blkio.throttle.io_service_bytes
```

El colector debe detectar automáticamente qué versión de cgroups está disponible.

#### 6.4 Compatibilidad con Podman

Podman expone una API compatible con Docker en:
```
/run/user/{uid}/podman/podman.sock   # modo rootless
/run/podman/podman.sock              # modo root
```

bollard puede conectarse a cualquier socket Unix, por lo que es compatible. La lógica de detección:

1. Intentar `/var/run/docker.sock` (Docker)
2. Si falla, intentar `/run/podman/podman.sock`
3. Si falla, intentar `/run/user/{uid}/podman/podman.sock`
4. Si todo falla, mostrar la pestaña de contenedores como "no disponible" con mensaje explicativo

#### 6.5 Widget de tabla de contenedores

```
  Contenedores  (Docker · 4 activos)                    Filtrar: /
  ──────────────────────────────────────────────────────────────────
  nginx-prod    0.1%   45MB/512MB   ↓1.2MB/s  R 0B/s   ● activo
  api-service   0.8%  128MB/512MB   ↓0.3MB/s  R 0.5MB/s ● activo
  redis         0.0%   12MB/256MB   ↓0B/s     R 0B/s    ● activo
  postgres-db   2.3%  312MB/1GB     ↓0B/s     W 2.1MB/s ● activo
  ──────────────────────────────────────────────────────────────────
  ↑↓ navegar   Enter detalle   / filtrar   R reiniciar   S detener
```

Columnas: Nombre, CPU%, RAM (usado/límite), Red entrada, Disco R/W, Estado.

#### 6.6 Sistema de pestañas

La parte inferior del layout tiene pestañas:

```
  [ Procesos ]  [ Contenedores ]
```

`Tab` alterna entre ellas. La pestaña activa se muestra con fondo cian.

Si Docker/Podman no está disponible, la pestaña de Contenedores muestra:
```
  Docker / Podman no detectado.
  Asegúrate de que el socket esté accesible:
    /var/run/docker.sock  o  /run/user/{uid}/podman/podman.sock
```

#### 6.7 Criterio de éxito de Fase 6

- La tabla de contenedores muestra CPU, RAM, red y I/O de disco en tiempo real
- El I/O de disco se lee correctamente de cgroups v2
- Funciona con Docker y Podman
- Si el runtime de contenedores no está disponible, muestra un mensaje claro en vez de crashear
- `Tab` alterna entre procesos y contenedores sin problemas

---

## 12. Fase 7 — Vistas de detalle

**Duración estimada:** 3–4 días  
**Objetivo:** Al presionar `Enter` sobre un proceso o contenedor, mostrar una vista expandida con todas sus métricas.

### Tareas

#### 7.1 Vista de detalle de proceso

Presionar `Enter` sobre un proceso en la tabla:

```
  ─── Proceso: postgres  ──────────────────────────────
  PID: 1842     Usuario: postgres     Threads: 14
  Uptime: 3 días, 2 horas

  CPU                                          2.3%
  ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░

  Memoria                              312 MB / 8 GB
  ████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░

  Disco
    Lectura                              4.2 MB/s
    ██████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
    Escritura                            1.1 MB/s
    ██░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░

  ─────────────────────────────────────────────────
  ESC volver
```

#### 7.2 Vista de detalle de contenedor

Presionar `Enter` sobre un contenedor:

```
  ─── Contenedor: api-service ─────────────────────────
  ID: a1b2c3d4e5f6     Imagen: node:20-alpine
  Estado: ● activo     Uptime: 2 días, 4 horas

  CPU                                          0.8%
  ██░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░

  Memoria                             128 MB / 512 MB
  ███░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░

  Red
    Entrada                              1.2 MB/s
    Salida                               0.3 MB/s

  Disco
    Lectura                              0.5 MB/s
    Escritura                            2.1 MB/s

  Puertos:   3000 → 3000/tcp
  Volúmenes: /host/data → /app/data

  ─────────────────────────────────────────────────
  ESC volver    L ver logs    R reiniciar    S detener
```

#### 7.3 Acción: ver logs del contenedor

Presionar `L` en la vista de detalle de contenedor abre una vista de logs:

```
  ─── Logs: api-service ───────────────────────────────
  [2024-01-15 14:32:01] Server listening on port 3000
  [2024-01-15 14:32:01] Database connected
  [2024-01-15 14:32:15] GET /api/health 200 2ms
  [2024-01-15 14:32:45] POST /api/data 201 45ms
  ...

  ESC volver    F seguir (tail)    ↑↓ scroll
```

Los logs se obtienen via `bollard::container::LogsOptions` con `follow: false` para obtener los últimos N logs, y `follow: true` para tail en tiempo real.

#### 7.4 Acciones: reiniciar y detener contenedor

`R` en la vista de detalle pide confirmación:
```
  ¿Reiniciar api-service?  [ Sí - Enter ]  [ No - ESC ]
```

`S` (stop) igual con confirmación. Se implementan via bollard (`docker.restart_container()`, `docker.stop_container()`).

#### 7.5 Criterio de éxito de Fase 7

- La vista de detalle de proceso muestra todas las métricas expandidas
- La vista de detalle de contenedor muestra todas las métricas incluyendo puertos y volúmenes
- Los logs funcionan en modo estático y modo tail
- Reiniciar y detener piden confirmación antes de ejecutar

---

## 13. Fase 8 — Intervalo de refresco configurable

**Duración estimada:** 1–2 días  
**Objetivo:** Permitir al usuario cambiar el intervalo de refresco en tiempo real sin reiniciar la aplicación.

### Tareas

#### 8.1 Control visual en el header

```
  rtop         srv-prod · Linux    Refresco: [ ◀  2s  ▶ ]    14:32:01
```

El control siempre visible en el header, a la derecha del hostname.

#### 8.2 Intervalos disponibles

Secuencia de valores al presionar `◀` / `▶` (o `[` / `]`):

```
0.5s → 1s → 2s → 5s → 10s → 30s → 60s
```

Al llegar al extremo de la secuencia, se detiene (no hay wrap).

#### 8.3 Implementación con tokio

El intervalo de polling se implementa con `tokio::time::interval()`. Para cambiarlo en caliente:

```rust
// En lugar de un interval fijo, usar un channel que envíe el nuevo intervalo
let (interval_tx, mut interval_rx) = tokio::sync::watch::channel(2.0f64);

// El collector loop lee el watch channel
tokio::spawn(async move {
    let mut current_interval = tokio::time::interval(Duration::from_secs_f64(2.0));
    loop {
        tokio::select! {
            _ = current_interval.tick() => { /* refrescar datos */ },
            Ok(()) = interval_rx.changed() => {
                let new_secs = *interval_rx.borrow();
                current_interval = tokio::time::interval(Duration::from_secs_f64(new_secs));
            }
        }
    }
});
```

#### 8.4 Criterio de éxito de Fase 8

- Presionar `[` y `]` cambia el intervalo de refresco inmediatamente
- El control visual muestra el valor actual en todo momento
- El cambio afecta a todos los colectores simultáneamente
- El valor elegido se guarda en el archivo de configuración

---

## 14. Fase 9 — Configuración persistente

**Duración estimada:** 1–2 días  
**Objetivo:** Guardar las preferencias del usuario entre reinicios.

### Tareas

#### 9.1 Estructura de configuración

En `src/config.rs`:

```rust
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub refresh_interval_secs: f64,
    pub selected_disk: Option<String>,
    pub selected_nic: Option<String>,
    pub default_tab: Tab,
    pub process_sort_column: SortColumn,
    pub show_swap: bool,
    pub docker_socket_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval_secs: 2.0,
            selected_disk: None,
            selected_nic: None,
            default_tab: Tab::Processes,
            process_sort_column: SortColumn::Cpu,
            show_swap: true,
            docker_socket_path: None,
        }
    }
}
```

#### 9.2 Ubicación del archivo

```
~/.config/rtop/config.toml
```

Seguir el estándar XDG. Usar `dirs` crate para obtener el config dir de forma multiplataforma.

#### 9.3 Carga y guardado

- Al iniciar: intentar cargar `config.toml`. Si no existe, usar `Config::default()` y crear el archivo.
- Al cambiar cualquier preferencia (disco, NIC, intervalo): guardar automáticamente sin interrupción del usuario.
- Si el archivo tiene errores de parseo: loggear el error, usar defaults, y no sobreescribir el archivo (preservar el archivo corrupto para que el usuario pueda inspeccionarlo).

#### 9.4 Criterio de éxito de Fase 9

- Las preferencias persisten entre reinicios
- El archivo se crea automáticamente en el primer uso
- Un archivo de config corrupto no impide que rtop arranque

---

## 15. Fase 10 — Polish de UX y accesibilidad visual

**Duración estimada:** 3–4 días  
**Objetivo:** Pulir la experiencia visual y de interacción para que sea intuitiva para cualquier usuario.

### Tareas

#### 10.1 Manejo de terminales pequeñas

Cuando la terminal es muy pequeña (< 80 columnas o < 24 filas):

```
  Terminal muy pequeña.
  Mínimo recomendado: 80×24
  Actual: 72×18
```

Detectar el tamaño con `terminal.size()` y mostrar este mensaje en vez de intentar renderizar el layout normal.

#### 10.2 Indicadores de carga

Al iniciar, mientras los colectores aún no tienen datos:

```
  CPU    [cargando...]
  RAM    [cargando...]
```

Evitar mostrar 0% o datos vacíos que puedan confundir.

#### 10.3 Indicadores de error claros

Si un colector falla (ej: se perdió la conexión con Docker):

```
  Contenedores  ⚠ Sin conexión con Docker — reintentando...
```

El colector reintenta automáticamente con backoff exponencial. El usuario no necesita hacer nada.

#### 10.4 Animación de actualización sutil

Un pequeño indicador en el header que parpadea brevemente cuando los datos se actualizan:

```
  rtop  srv-prod  •  14:32:01  ●    ← el punto parpadea en cada refresh
```

Implementado con un estado bool que alterna en cada actualización de datos.

#### 10.5 Pantalla de ayuda (F1)

```
  ─── rtop — Ayuda ──────────────────────────────────────
  
  Navegación
    Tab / Shift+Tab    Cambiar pestaña (Procesos/Contenedores)
    ↑ ↓                Navegar en la lista
    Enter              Ver detalle del proceso/contenedor
    ESC                Volver / Cerrar modal
  
  Sistema
    F2                 Elegir dispositivo de disco
    F3                 Elegir interfaz de red
    [ / ]              Reducir/aumentar intervalo de refresco
  
  Procesos
    c                  Ordenar por CPU
    m                  Ordenar por memoria
    r                  Ordenar por lectura de disco
    w                  Ordenar por escritura de disco
    /                  Filtrar por nombre
  
  Contenedores (detalle)
    L                  Ver logs
    R                  Reiniciar contenedor
    S                  Detener contenedor
  
  General
    F1                 Mostrar/ocultar esta ayuda
    q / Ctrl+C         Salir
  
  ─────────────────────────────────────────────────────────
  ESC cerrar ayuda
```

#### 10.6 Formato de números consistente

Reglas para formatear valores:

- **Bytes/s**: usar `bytesize` crate → `"1.2 MB/s"`, `"450 KB/s"`, `"0 B/s"`
- **Porcentajes**: siempre 1 decimal → `"2.3%"`, `"100.0%"`
- **Bytes absolutos**: `"312 MB"`, `"1.2 GB"`, `"45 KB"`
- **Tiempo**: `"3d 2h"`, `"45m"`, `"30s"`
- **Sin dato / sin permiso**: mostrar `"–"` (guión largo), nunca `"0"` cuando no se sabe

#### 10.7 Criterio de éxito de Fase 10

- rtop se ve bien en terminales de 80×24, 120×35, y 200×50
- Los errores de colectores se muestran de forma no intrusiva
- La pantalla de ayuda es completa y clara
- Todos los números están formateados de forma consistente

---

## 16. Fase 11 — Empaquetado y release

**Duración estimada:** 3–4 días  
**Objetivo:** Publicar rtop de forma que cualquiera pueda instalarlo fácilmente.

### Tareas

#### 16.1 Compilación optimizada

```toml
[profile.release]
opt-level = 3
lto = true          # Link-Time Optimization
codegen-units = 1   # Mejor optimización, compile time más lento
strip = true        # Eliminar símbolos de debug del binario final
```

El binario resultante debería ser < 5 MB y arrancar en < 100ms.

#### 16.2 GitHub Actions CI

En `.github/workflows/ci.yml`:

```yaml
on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
      - run: cargo build --release
```

#### 16.3 Releases automáticos con binarios

En `.github/workflows/release.yml`, al hacer push de un tag `v*`:

- Compilar para `x86_64-unknown-linux-gnu`
- Compilar para `aarch64-unknown-linux-gnu` (ARM, instancias Graviton de AWS)
- Compilar para `x86_64-apple-darwin`
- Compilar para `aarch64-apple-darwin` (Apple Silicon)
- Publicar los binarios en GitHub Releases

#### 16.4 Instalación via cargo

```bash
cargo install rtop
```

Publicar en `crates.io`.

#### 16.5 Script de instalación rápida

```bash
curl -fsSL https://github.com/usuario/rtop/install.sh | sh
```

El script detecta la arquitectura, descarga el binario correcto de GitHub Releases, y lo instala en `/usr/local/bin/`.

#### 16.6 Paquete para distribuciones Linux

- `.deb` para Debian/Ubuntu: usando `cargo-deb`
- `.rpm` para RHEL/CentOS/Fedora: usando `cargo-rpm`
- Fórmula Homebrew para macOS

#### 16.7 README completo

El README debe incluir:

- GIF animado mostrando rtop en acción (grabado con `asciinema`)
- Sección de instalación con todos los métodos
- Tabla de diferencias vs btop, ctop, htop
- Sección de configuración con ejemplo de `config.toml`
- Sección de solución de problemas comunes (Docker socket, permisos de `/proc`)
- Badges: CI, versión en crates.io, licencia

---

## 17. Referencia de atajos de teclado

| Tecla | Acción |
|-------|--------|
| `q` / `Ctrl+C` | Salir |
| `Tab` | Cambiar pestaña (Procesos ↔ Contenedores) |
| `↑` / `↓` | Navegar en lista |
| `Enter` | Ver detalle |
| `ESC` | Volver / Cerrar modal |
| `F1` | Pantalla de ayuda |
| `F2` | Selector de disco |
| `F3` | Selector de interfaz de red |
| `[` | Reducir intervalo de refresco |
| `]` | Aumentar intervalo de refresco |
| `c` | Ordenar procesos por CPU |
| `m` | Ordenar procesos por memoria |
| `r` | Ordenar procesos por lectura de disco |
| `w` | Ordenar procesos por escritura de disco |
| `/` | Filtrar por nombre |
| `L` | Ver logs (en detalle de contenedor) |
| `R` | Reiniciar contenedor (con confirmación) |
| `S` | Detener contenedor (con confirmación) |

---

## 18. Diseño visual del layout

### Vista principal completa

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

## 19. Fuentes de datos técnicas

| Métrica | Fuente en Linux | Crate / Método |
|---------|----------------|----------------|
| CPU global | `/proc/stat` | `sysinfo` |
| CPU por core | `/proc/stat` | `sysinfo` |
| RAM total/usado | `/proc/meminfo` | `sysinfo` |
| Swap | `/proc/meminfo` | `sysinfo` |
| Disco uso % | `/proc/mounts` + `statvfs` | `sysinfo` |
| Disco R/W sistema | `/proc/diskstats` | lectura directa |
| Disco R/W proceso | `/proc/{pid}/io` | lectura directa |
| Red ↑↓ | `/proc/net/dev` | `sysinfo` |
| Lista de procesos | `/proc/` | `sysinfo` |
| CPU por proceso | `/proc/{pid}/stat` | `sysinfo` |
| RAM por proceso | `/proc/{pid}/status` | `sysinfo` |
| Contenedores lista | Docker Engine API | `bollard` |
| CPU/RAM contenedor | Docker Engine API `/stats` | `bollard` |
| Red por contenedor | Docker Engine API `/stats` | `bollard` |
| Disco R/W contenedor (cgroups v2) | `/sys/fs/cgroup/.../io.stat` | lectura directa |
| Disco R/W contenedor (cgroups v1) | `/sys/fs/cgroup/blkio/docker/{id}/...` | lectura directa |

---

## 20. Criterios de éxito por fase

| Fase | Criterio principal |
|------|--------------------|
| 1. Scaffold | `cargo run` abre TUI, `q` cierra limpiamente |
| 2. Métricas | CPU, RAM, Disco % en tiempo real con colores semánticos |
| 3. Disco I/O | R/W en tiempo real, selector de dispositivo funciona en AWS EBS |
| 4. Red | ↑↓ en tiempo real, selector de NIC funciona |
| 5. Procesos | Tabla con CPU, RAM, Disco R/W por proceso, filtrado y ordenamiento |
| 6. Contenedores | Tabla con CPU, RAM, Red, Disco R/W por contenedor. Docker y Podman |
| 7. Detalle | Vista de proceso y contenedor completa, logs, reiniciar/detener |
| 8. Refresco | Cambio en caliente de intervalo con teclas `[` y `]` |
| 9. Config | Preferencias persisten entre reinicios |
| 10. UX | Funciona en 80×24, errores claros, ayuda completa |
| 11. Release | Binarios para Linux x86/ARM y macOS. `cargo install rtop` funciona |

---

## Notas finales

### Sobre el nombre `rtop`

Corto, directo, memorizable. La `r` comunica Rust para quienes lo conocen, y para quienes no, simplemente es un nombre de herramienta de sistema. No hay conflictos conocidos con herramientas existentes populares.

### Priorización si el tiempo es limitado

Si se necesita lanzar un MVP antes de completar todas las fases, el orden de prioridad es:

1. Fases 1–2: Base funcional (sin esto no hay nada)
2. Fase 3: Disco con I/O y selector (el diferenciador #1 vs btop)
3. Fase 6: Contenedores básicos (el diferenciador #2 vs todo lo demás)
4. Fase 5: I/O por proceso (complementa el diferenciador de disco)
5. Todo lo demás

### Licencia recomendada

MIT o Apache 2.0 (dual license). Es el estándar del ecosistema Rust y maximiza la adopción.

---

*Plan generado para el proyecto rtop — Monitor de sistema unificado en Rust*  
*Versión 1.0 del plan*
