# Documentação do SysMon

## Documentação atual

- [Arquitetura](architecture.md): estrutura do código, coleta, históricos, renderização e paletas.
- [Desenvolvimento](development.md): build, verificações, capturas, smoke e benchmarks.
- [Interface normal](assets/dashboard-normal.png), [atenção](assets/dashboard-warning.png) e [crítico](assets/dashboard-critical.png): capturas com telemetria sintética.

## Histórico

As decisões e medições das revisões gráficas ficam em
[archive/graphics-overhaul](archive/graphics-overhaul/README.md).
Elas documentam as versões medidas; não substituem as instruções atuais.

Arquivos gerados durante desenvolvimento devem ficar em `target/`. Somente
capturas de referência e evidências citadas na documentação são versionadas.


## Organização de 22/10/2026

Foram retirados o histórico privado antigo de 60 amostras, os tweens de métricas
que já não alimentavam a interface, `FieldDynamics` sem consumidores e helpers
internos sem uso. O motor atual passou para `src/pulse_v2/`; os caminhos públicos
da biblioteca foram mantidos. As permissões gerais que ocultavam código/imports
sem uso foram removidas dos módulos afetados.

Validação: 326 testes passaram, build release e Clippy estrito passaram; os
11 testes removidos pertenciam às implementações aposentadas. As 12 capturas por
célula ficaram idênticas às anteriores. Smoke em PTY passou em Unicode/True Color
e ASCII/mono, com restauração do terminal. Os links locais desta documentação
foram conferidos após a reorganização.

Três diretórios temporários de investigações anteriores foram removidos,
liberando aproximadamente 320 MiB. Capturas e resultados necessários às
comparações históricas foram preservados em `archive/graphics-overhaul/evidence/`.
