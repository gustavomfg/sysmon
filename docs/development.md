# Desenvolvimento

Use Rust 1.95 ou superior. As dependências e suas versões ficam em `Cargo.toml`
e `Cargo.lock`; não há dependência gráfica além da stack de terminal.

## Build e verificações

Na raiz do repositório:

```sh
cargo run
cargo fmt --check
cargo test --offline --all-targets
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline --release --bin sysmon --example graphics_capture
```

`--offline` exige dependências já disponíveis no cache do Cargo. Na primeira
compilação, use os comandos sem essa opção para obtê-las.

Os testes unitários ficam junto dos módulos em `src/`; `tests/telemetry_modes.rs`
confere valores conhecidos com a aplicação real em foco/expansão. Históricos de produção pertencem
a `ui::dashboard_v2`; o estado de comandos em `app.rs` não mantém uma cópia deles.

## Capturas reproduzíveis

O exemplo usa a aplicação real com `TestBackend` do ratatui e dados **sintéticos**.
Gera arquivos `.cells` e `.txt` para cinco tamanhos de terminal, três temas e os
principais modos expandidos. `--all-focus` acrescenta os dez painéis expandidos
em cada um dos cinco tamanhos:

```sh
target/release/examples/graphics_capture target/captures/normal --all-focus
target/release/examples/graphics_capture target/captures/warning --status=warning
target/release/examples/graphics_capture target/captures/critical --status=critical
python3 tools/render_graphics_capture.py target/captures/normal
```

O conversor opcional para PNG requer Pillow e uma das fontes indicadas no script.
Ele não instala dependências. Repita o comando de conversão para as outras pastas
quando precisar inspecionar estados de atenção e crítico.

## Execução real em terminal

```sh
python3 tools/smoke_graphics_terminal.py target/release/sysmon
```

O smoke cria um PTY e exercita foco, expansão, busca, troca de tema, resize e saída
em Unicode/True Color e ASCII/monocromático. Verifica a restauração do terminal.
Não executa a exportação de snapshots.

## Conferência dos coletores no Linux

```sh
cargo run --offline --release --example telemetry_check
```

A execução dura cerca de sete segundos, com três amostras após a inicialização.
Compara CPU, memória, swap, rede e I/O com `/proc`, capacidade com `df` e registra
as métricas disponíveis de GPU. Não grava hostname, nomes de processos ou conteúdo
da tela. Frequência e GPU são leituras dinâmicas, não medições sincronizadas de
laboratório; os limites e resultados estão na [auditoria](telemetry-audit.md).

## Benchmarks limitados

```sh
target/release/examples/graphics_capture target/bench/frozen --bench
target/release/examples/graphics_capture target/bench/animated --bench --animate
python3 tools/benchmark_graphics_terminal.py target/release/sysmon --width 160 --height 45
```

Os dois primeiros medem desenho com dados sintéticos; `--animate` também avança
a animação. O benchmark em PTY mede o processo real por oito segundos, descarta
dois de aquecimento e informa CPU relativa a um núcleo e RSS. Sua saída contém
estatísticas, sem conteúdo da tela. Para comparar versões, alterne a ordem das
execuções e mantenha dimensões, modo e ambiente iguais.

## Arquivos locais

`target/` contém binários, caches, capturas e resultados temporários. Diretórios
de investigação antigos podem ser removidos quando suas evidências necessárias
já estiverem versionadas. Mantenha as configurações do editor e os arquivos locais
de ambiente fora do Git. As opções de terminal estão no [README](../README.md#compatibilidade-de-terminal).
