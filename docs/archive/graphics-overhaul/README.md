# Histórico da revisão gráfica — outubro de 2026

- [Pesquisa](research.md): auditoria inicial e referências técnicas.
- [Resultados](results.md): entregas, capturas, medições e limitações de cada etapa.
- [Progresso](progress.md): registro de execução e retomadas.
- `evidence/`: CSV/JSON originais, capturas históricas e patch de instrumentação.

As imagens atuais da interface ficam em [docs/assets](../../assets/).
As imagens em `evidence/` representam versões anteriores e foram mantidas para
que as comparações documentadas continuem verificáveis.

Os caminhos `target/graphics-overhaul`, `target/pulse-performance-20261022` e
`target/palette-review` citados nos registros eram temporários e foram removidos
na organização do projeto. Os dados necessários permanecem neste arquivo histórico.

O patch `evidence/pulse-profiling.patch` se refere à árvore anterior à organização
do motor: `src/ui/widgets/pulse_v2/engine.rs`. Para reproduzir a investigação,
use o commit indicado no relatório em uma cópia temporária. O motor atual está
em `src/pulse_v2/`, com o mesmo caminho público `sysmon::pulse_v2`.
