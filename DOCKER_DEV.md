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

### 4. Simuladores de carga PSI (servicios independientes)

El `docker-compose.yml` incluye tres servicios dedicados, cada uno diseñado para
elevar exactamente **un** tipo de métrica PSI. Se arrancan de forma independiente
desde el host (sin necesidad de entrar al contenedor):

#### Estrés de CPU → sube `PSI cpu`
```bash
docker compose run --rm stress-cpu
```
Lanza `stress-ng --cpu 0 --cpu-load 90` (todos los núcleos al 90 %).
Mientras corre, observa `some`/`full` en `/proc/pressure/cpu` subir en `rtop`.

#### Estrés de Memoria → sube `PSI memory`
```bash
docker compose run --rm stress-mem
```
Lanza 4 workers de VM con 256 MB cada uno (`--vm-hang 0` para forzar stalls).
Observa `/proc/pressure/memory` en `rtop`.

#### Estrés de I/O → sube `PSI io`
```bash
docker compose run --rm stress-io
```
Lanza 4 workers de I/O mixto con 512 MB (`--iomix`).
Observa `/proc/pressure/io` en `rtop`.

> **Flujo de simulación recomendado:**
> 1. Terminal A: `docker compose exec -it dev bash` → `cargo run` (rtop en vivo)
> 2. Terminal B: `docker compose run --rm stress-cpu` (o `stress-mem` / `stress-io`)
> 3. Observa en rtop cómo suben los gráficos PSI correspondientes en tiempo real.
> 4. Ctrl+C en Terminal B para detener la carga; los valores PSI vuelven a bajar.

#### Estrés manual desde dentro del contenedor (alternativa)

Si prefieres lanzar `stress-ng` directamente desde la shell de `dev`:

```bash
# CPU
stress-ng --cpu 4 --cpu-load 90 --timeout 60s

# Memoria
stress-ng --vm 4 --vm-bytes 256M --timeout 60s

# I/O
stress-ng --iomix 4 --iomix-bytes 512M --timeout 60s
```

## Detener el entorno

```bash
docker compose down
```

Los volúmenes **no** se eliminan (los binarios compilados y la caché de crates
se conservan). Para limpiarlos por completo:

```bash
docker compose down -v
```
