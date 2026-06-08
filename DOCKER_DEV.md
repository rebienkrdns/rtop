# Entorno de Desarrollo con Docker Compose

Guía para compilar, ejecutar y probar `rtop` dentro de un contenedor Linux nativo desde macOS.

## Requisitos

- Docker Desktop ≥ 4.x (incluye `docker compose` sin guion)
- No se necesita Rust instalado en el host

## Inicio rápido

### 1. Levantar el entorno en segundo plano

```bash
docker compose up -d
```

El contenedor descarga la imagen `rust:latest` (Debian), instala `stress-ng` y queda
en espera. Los volúmenes nombrados se crean automáticamente:

| Volumen        | Ruta en contenedor            | Propósito                                 |
|----------------|-------------------------------|-------------------------------------------|
| `cargo-target` | `/app/target`                 | Binarios Linux; no colisiona con macOS    |
| `cargo-cache`  | `/usr/local/cargo/registry`   | Caché de crates; persiste entre sesiones  |

### 2. Entrar a la shell interactiva

```bash
docker compose exec -it dev bash
```

Una vez dentro del contenedor:

```bash
# Compilar en modo debug
cargo build

# Ejecutar rtop (TUI interactivo)
cargo run

# Compilar en modo release
cargo build --release
```

### 3. Verificar las métricas PSI del kernel

El kernel Linux expuesto por la VM de Docker Desktop soporta PSI.
Comprueba la disponibilidad desde dentro del contenedor:

```bash
# Si los archivos existen y tienen contenido, PSI está activo
cat /proc/pressure/cpu
cat /proc/pressure/memory
cat /proc/pressure/io
```

Salida esperada (valores de ejemplo):

```
some avg10=0.00 avg60=0.00 avg300=0.00 total=0
full avg10=0.00 avg60=0.00 avg300=0.00 total=0
```

> **Nota:** En Docker Desktop para macOS, los archivos `/proc/pressure/*` existen
> dentro de la VM Linux que Docker gestiona. Si no aparecen, asegúrate de tener
> Docker Desktop ≥ 4.6 con el kernel de VM actualizado.

### 4. Generar carga para probar los gráficos PSI

`stress-ng` se instala automáticamente al arrancar el contenedor:

```bash
# Estresar CPU durante 60 s (4 workers)
stress-ng --cpu 4 --timeout 60s

# Estresar memoria
stress-ng --vm 2 --vm-bytes 512M --timeout 60s

# Estresar I/O
stress-ng --io 4 --timeout 60s
```

Mientras corre `stress-ng` en una sesión, abre otra sesión con
`docker compose exec -it dev bash` y ejecuta `cargo run` para observar
los gráficos PSI en tiempo real.

## Detener el entorno

```bash
docker compose down
```

Los volúmenes **no** se eliminan (los binarios compilados y la caché de crates
se conservan). Para limpiarlos por completo:

```bash
docker compose down -v
```
