# Sysmon

Um monitor de sistema no terminal feito em Rust, com interface TUI leve, atualização em tempo real e visual de terminal para acompanhar CPU, RAM, GPU, discos, processos e rede.

![Rust](https://img.shields.io/badge/Rust-2024-orange?style=flat-square)
![TUI](https://img.shields.io/badge/UI-ratatui-blue?style=flat-square)
![Status](https://img.shields.io/badge/status-active-brightgreen?style=flat-square)

## Preview

![Dashboard com instrumentos de telemetria](docs/assets/dashboard-normal.png)

Captura do renderer real a 160×45, com dados sintéticos identificados do exemplo de desenvolvimento.

## Recursos

- Dashboard TUI usando `ratatui` e `crossterm`.
- Linha vital no cabeçalho: um traço de eletrocardiograma cujo ritmo (bpm), amplitude e onda T seguem o System Pulse, com diagnóstico em linguagem natural do que está ditando o ritmo.
- Tabela de processos sem threads duplicadas, com CPU por processo medida no próprio intervalo de coleta, texto branco e medidor por linha.
- Mapa de núcleos no card de CPU: uma barra por núcleo lógico, revelando núcleos saturados mesmo quando a média está baixa.
- Instrumentos de telemetria em Braille, com timestamps reais, escalas, picos e preenchimento discreto.
- Narrador de eventos: mudanças de saúde que persistem (ex.: "CPU under sustained load", "Back to calm") entram em RECENT EVENTS com idade relativa.
- Painéis com foco branco e superfícies neutras no tema Pulse; verde para telemetria normal, amarelo para atenção e vermelho para estado crítico.
- CPU com escala fixa de 0–100%, comparação em pontos percentuais e detalhamento por núcleo no modo expandido.
- Informações de sistema: OS, kernel, uptime, cores, load average e quantidade de processos.
- Contexto operacional ao vivo com hostname, memória disponível, idade da amostra e identidade dos processos.
- RAM com barra de capacidade, memória disponível, histórico e contexto de swap quando disponível.
- Top 5 processos por uso de CPU, exibindo também PID e memória.
- Armazenamento com capacidade separada da atividade histórica de leitura/escrita.
- Rede com download/upload, picos recentes e escala compartilhada; interfaces adicionais aparecem nos detalhes.
- Métricas da GPU principal via `nvidia-smi`, quando disponível, inclusive em foco e expansão.
- Layout responsivo, com aviso quando o terminal não possui o tamanho mínimo necessário.
- Três temas, barras finas e cores consistentes; saída alternativa em ASCII, 16/256 cores ou monocromática.
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

Esse é o único comando de execução e inicia o dashboard atual do SysMon.

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
| `j` / `k` ou `↑` / `↓` | Navegar entre widgets |
| `e` | Expandir/recolher o widget focado |

Na command palette, use as setas para navegar, `Enter` para executar e `Esc` para fechar. A opção `Settings` permite trocar o tema, reduzir o movimento e ajustar o intervalo de atualização.

## Como Funciona

O app coleta as métricas no intervalo configurado (1, 2 ou 4 segundos) e mantém a animação/renderização separada da coleta. Eventos de mouse não são capturados, evitando picos falsos de CPU ao mover ou rolar o mouse no terminal.

A coleta principal vem da crate `sysinfo`. Para a GPU, o app chama `nvidia-smi`, valida as métricas retornadas e seleciona a placa principal. O painel normal, o foco e a expansão mostram exclusivamente as métricas e o histórico dessa placa.

## Limitações

- GPUs são detectadas apenas em placas NVIDIA com `nvidia-smi` disponível.
- Rede e I/O aparecem em unidades binárias explícitas (`KiB/s`, `MiB/s`); startup e resets de rede produzem lacunas, sem taxas artificiais.
- O histórico retém 120 amostras por série. Os cards mostram os últimos dois minutos disponíveis; a expansão aproveita o histórico retido. Alterar a largura não muda a janela temporal.
- O painel e o histórico de GPU acompanham apenas a placa principal. O histórico de armazenamento acompanha o dispositivo principal, com valores atuais dos demais discos disponíveis. Rede agrega interfaces que não são loopback, incluindo interfaces virtuais.
- Sensores sem limite crítico informado pelo driver usam uma escala visual neutra. As heurísticas gerais de saúde do SYSTEM PULSE continuam sendo as existentes.
- O layout ajusta a proporção dos painéis conforme a largura; abaixo do tamanho mínimo, o app exibe um aviso em vez de cortar o dashboard.

## Desenvolvimento

As instruções de testes, capturas sintéticas, smoke em terminal e benchmarks
estão em [Desenvolvimento](docs/development.md). A [documentação](docs/README.md)
reúne a arquitetura atual e as evidências históricas das revisões.

### Compatibilidade de terminal

Detecção padrão preserva Unicode e True Color em terminais modernos; `TERM=linux` ou `dumb` usa ASCII e 16 cores. Para configurar explicitamente:

```bash
SYSMON_COLOR=ansi256 cargo run --release
SYSMON_COLOR=mono SYSMON_GRAPHICS=ascii cargo run --release
```

`SYSMON_COLOR` aceita `truecolor`, `ansi16`, `ansi256` e `mono`; `SYSMON_GRAPHICS` aceita `unicode` e `ascii`. `NO_COLOR` desativa cores quando não há escolha explícita em `SYSMON_COLOR`.

## Stack

- Rust 2024
- `ratatui`
- `crossterm`
- `sysinfo`

## Licença

Este projeto é disponibilizado sob a licença MIT.
