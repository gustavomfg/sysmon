# Sysmon

Um monitor de sistema no terminal feito em Rust, com interface TUI leve, atualização em tempo real e visual de terminal para acompanhar CPU, RAM, GPU, discos, processos e rede.

![Rust](https://img.shields.io/badge/Rust-2024-orange?style=flat-square)
![TUI](https://img.shields.io/badge/UI-ratatui-blue?style=flat-square)
![Status](https://img.shields.io/badge/status-active-brightgreen?style=flat-square)

## Preview

```text
┌─ SYSMON ────────────────────────────┐ ┌─ GPU ───────────────────────────────┐
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
- Layout responsivo, com aviso quando o terminal não possui o tamanho mínimo necessário.
- Palette de cores discreta, bordas quadradas e medidores ASCII para manter a leitura com cara de terminal.
- Primeira coleta real antes do desenho inicial, sem valores de demonstração na interface principal.
- Command palette, busca de processos, foco por widget e modo de movimento reduzido.
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
| `Esc` | Fechar o painel atual ou sair |
| `Ctrl+C` | Sair |
| `Ctrl+P` | Abrir a command palette |
| `/` ou `Ctrl+F` | Buscar processos |
| `Tab` / `Shift+Tab` | Alternar o foco entre widgets |

Na command palette, use as setas para navegar, `Enter` para executar e `Esc` para fechar. A opção `Settings` permite trocar o tema, reduzir o movimento e ajustar o intervalo de atualização.

## Como Funciona

O app coleta as métricas no intervalo configurado (1, 2 ou 4 segundos) e mantém a animação/renderização separada da coleta. Eventos de mouse não são capturados, evitando picos falsos de CPU ao mover ou rolar o mouse no terminal.

A coleta principal vem da crate `sysinfo`. Para GPUs, o app chama `nvidia-smi`, valida as métricas retornadas e exibe todas as placas com dados válidos quando o comando está disponível e retorna com sucesso. Uma linha inválida não impede a exibição das demais GPUs.

## Limitações

- GPUs são detectadas apenas em placas NVIDIA com `nvidia-smi` disponível.
- A leitura de rede divide o delta de bytes pelo intervalo real entre atualizações e apresenta a taxa em `MB/s` decimal.
- O layout ajusta a proporção dos painéis conforme a largura; abaixo do tamanho mínimo, o app exibe um aviso em vez de cortar o dashboard.

## Desenvolvimento

Comandos úteis:

```bash
cargo fmt
cargo check
cargo clippy -- -D warnings
cargo test
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
