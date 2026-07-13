# Sysmon

Um monitor de sistema no terminal feito em Rust, com interface TUI leve, atualização em tempo real e visual limpo para acompanhar CPU, RAM, GPU, discos, processos e rede.

![Rust](https://img.shields.io/badge/Rust-2024-orange?style=flat-square)
![TUI](https://img.shields.io/badge/UI-ratatui-blue?style=flat-square)
![Status](https://img.shields.io/badge/status-active-brightgreen?style=flat-square)

## Preview

```text
┌ SYSTEM ─────────────────────────────┐ ┌ GPU ────────────────────────────────┐
│ Linux | Kernel 6.x                  │ │ NVIDIA GeForce ...                  │
│ CPU: AMD/Intel ...                  │ │ Temp: 52°C                           │
│ Cores: 8 | Processos: 240           │ │ Uso: 15 %                            │
│ Uptime: 2d 4h 12m                   │ │ VRAM: 1024 MiB                       │
└─────────────────────────────────────┘ └─────────────────────────────────────┘
┌ RAM 5.2/15.5GB ─────────────────────┐ ┌ TOP MEMORY ─────────────────────────┐
│ █████████░░░░░░░░░░░ 34%             │ │ firefox 820.4MB                      │
└─────────────────────────────────────┘ └─────────────────────────────────────┘
┌ DISKS ─────────┐ ┌ CPU HISTORY ─────┐ ┌ NETWORK ────────────────────────────┐
│ / / 48/220GB   │ │ ▂▃▅▇▆▃▂          │ │ wlan0 ↓ 0.32MB/s ↑ 0.04MB/s          │
└────────────────┘ └──────────────────┘ └─────────────────────────────────────┘
```

## Recursos

- Dashboard TUI usando `ratatui` e `crossterm`.
- Uso geral de CPU com histórico visual.
- Informações de sistema: OS, kernel, uptime, cores, load average e quantidade de processos.
- RAM e swap usados/totais com barra de progresso.
- Top 5 processos por uso de memória, exibindo também CPU por processo.
- Discos montados com espaço usado/total.
- Rede por interface em `MB/s`, priorizando as interfaces com mais tráfego.
- GPUs NVIDIA via `nvidia-smi`, quando disponível, exibindo todas as placas detectadas.
- Saída segura com restauração do terminal.

## Requisitos

- Rust instalado.
- Linux/macOS/Windows com terminal compatível.
- Opcional: `nvidia-smi` para mostrar dados de GPU NVIDIA.

## Instalação

Clone o repositório:

```bash
git clone https://github.com/gustavomfg/sysmon.git
cd sysmon
```

Execute em modo desenvolvimento:

```bash
cargo run
```

Ou gere o binário otimizado:

```bash
cargo build --release
```

O executável ficará em:

```bash
target/release/sysmon
```

Para instalar localmente com Cargo:

```bash
cargo install --path .
```

Depois execute:

```bash
sysmon
```

## Atalhos

| Tecla | Ação |
| --- | --- |
| `q` | Sair |
| `Esc` | Sair |
| `Ctrl+C` | Sair |

## Como Funciona

O app atualiza as métricas em intervalos de 1 segundo. Eventos de mouse não são capturados, evitando picos falsos de CPU ao mover ou rolar o mouse no terminal.

A coleta principal vem da crate `sysinfo`. Para GPUs, o app chama `nvidia-smi`, valida as métricas retornadas e exibe todas as placas com dados válidos quando o comando está disponível e retorna com sucesso. Uma linha inválida não impede a exibição das demais GPUs.

## Limitações

- GPUs são detectadas apenas em placas NVIDIA com `nvidia-smi` disponível.
- A leitura de rede divide o delta de bytes pelo intervalo real entre atualizações e apresenta a taxa em `MB/s` decimal.
- O layout é otimizado para terminais médios/grandes; janelas muito pequenas podem cortar texto.

## Desenvolvimento

Comandos úteis:

```bash
cargo fmt
cargo check
cargo clippy -- -D warnings
```

Build limpo, se necessário:

```bash
cargo clean
cargo build --release
```

## Stack

- Rust 2024
- `ratatui`
- `crossterm`
- `sysinfo`

## Licença

Este projeto é disponibilizado sob a licença MIT.
