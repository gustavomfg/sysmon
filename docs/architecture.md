# Arquitetura — Telemetry Instruments

## Organização do código

- `src/main.rs`: terminal, eventos e execução da coleta em background.
- `src/system/`: coleta e normalização de hardware e sistema operacional.
- `src/ui/dashboard.rs`: estado de execução, navegação e overlays.
- `src/ui/dashboard_v2/`: modelos, históricos temporais e instrumentos de terminal.
- `src/pulse_v2/`: motor independente de partículas, campos e rasterização.
- `src/theme/`: paletas e política de texto.
- `src/app.rs` e módulos associados: comandos, configuração, foco, busca e eventos.
- `src/animation/`: utilitário público `Tween`, mantido para compatibilidade da biblioteca.

O estado de interação retém o snapshot necessário para busca/exportação, mas não
calcula históricos ou tweens de métricas. O único histórico de telemetria em uso
é `LiveMetricsHistory`; o movimento do Pulse usa os motores de saúde e comportamento.
O módulo `crate::pulse_v2` e os reexports existentes continuam públicos.

## Fluxo e responsabilidades

```text
SystemInfoCollector (sysinfo / nvidia-smi)
  → SystemInfo → LiveSnapshot / HardwareInventory
  → LiveMetricsHistory (oito séries limitadas)
  → modelos de CPU, memória, GPU, rede e armazenamento
  → WidgetRenderModel compartilhado (Arc)
  → TerminalRenderer / instrumentos / Buffer ratatui
  → adaptação opcional de cores e alfabeto → Crossterm
```

Coleta: 1/2/4 segundos, GPU com cache de 5 segundos. Loop visual: intervalo de
50 ms; o SYSTEM PULSE conserva seu próprio relógio e cache. Os instrumentos
laterais atualizam seus modelos na coleta e seus pixels quando conteúdo,
foco, tema, inventário, expansão ou geometria mudam.

O cache da cena animada do Pulse usa ticks de 100 ms. Desde a correção de
22/10/2026, mudanças intermediárias das métricas suavizadas são consolidadas:
o primeiro frame disponível no próximo tick consome a entrada mais recente.
Isso mantém a geração em aproximadamente 10 Hz, com a UI em até 20 Hz. Tamanho,
tema e modo de movimento invalidam imediatamente; no movimento reduzido, uma
nova entrada também invalida, pois o tick fica parado. A regra altera somente
a reutilização da cena na integração; o motor e seus algoritmos são os mesmos.

## Histórico e tempo

`chart.rs` mantém `TimeSeries<N>` em array circular, com escrita de slot e
evicção O(1), ordem lógica preservada e guarda de monotonicidade mesmo após
expiração. O cálculo do pico da escala de taxas faz uma varredura limitada ao
buffer. Não há armazenamento crescente. A configuração atual retém 120 amostras
por série; isso representa aproximadamente 2/4/8 minutos conforme a cadência.

`SparklineModel` conserva até 120 valores originais, bitmap de disponibilidade,
idades em nanossegundos e offset monotônico da última amostra. Não utiliza
frames para estimar tempo. A apresentação padrão usa 120 segundos; gráficos
altos usam o histórico retido, arredondando a janela para minutos. Largura muda
resolução, sem recortar a duração. O rodapé `last` indica a última amostra, não
o relógio atual; a idade da coleta continua no cabeçalho da aplicação.

Lacunas interrompem linha e preenchimento. Intervalos acima de
`max(10 s, 3 × cadência mediana)` também interrompem a conexão. Uma única
amostra gera um ponto, com espaço anterior vazio. Não há média móvel aplicada
silenciosamente nem preenchimento de dados ausentes.

## Escalas e coleta

- CPU, RAM e GPU: domínio normalizado 0–1, rótulo 0–100%.
- Rede/I/O: baseline zero, teto em potências de dois, mínimo 1 KiB/s.
  Cresce imediatamente; desce após 30 s com pico recente abaixo de um quarto
  do teto. Lacunas e pausas reiniciam a comprovação desse intervalo.
- As duas direções usam o mesmo teto e janela. Histórico expandido ainda pode
  aumentar o teto para acomodar picos antigos realmente retidos.
- Bytes e taxas exibem unidades binárias explícitas; conversões do coletor
  legado em MB/s decimal ocorrem na adaptação para bytes/s.
- CPU e CPU por processo: startup, intervalo insuficiente e reset não são 0%;
  são dados indisponíveis. Os campos públicos `f32` usam NaN nesse caso; a UI
  mostra `—` e a exportação JSON escreve `null`. CPU por processo verifica também
  o instante de criação, para não reutilizar contadores de um PID reciclado.
- Disco no Linux: capacidade agrupa sistemas de arquivos montados, deduplicando
  aliases; I/O mede o dispositivo indicado usando `/sys/class/block/<id>/stat`
  e deltas monotônicos. Primeira leitura, dispositivo novo e reset produzem lacunas.
  IDs como `dm-0`, `md127`, `loop0` e `zram0` não perdem seus sufixos numéricos.
- Rede: delta de bytes sobre intervalo monotônico real. Startup, interface nova,
  reset/wrap ou intervalo nulo tornam a taxa indisponível. Mudança de membros
  da agregação inicia uma nova época de histórico. Não loopback inclui interfaces
  virtuais; não é uma agregação exclusivamente física.
- Trocar GPU/disco principal inicia uma série nova. Inventário e valores atuais
  de múltiplos dispositivos continuam disponíveis.
- Temperaturas preservam valores reais. Somente limites críticos informados
  pelo driver geram o indicador térmico de atenção/limite; sem limite, a barra
  é uma régua neutra. As heurísticas gerais de saúde/Pulse são preservadas.

## Renderização

`terminal/history_chart.rs` reutiliza o rasterizador original: 2×4 subpixels
Braille por célula, segmentos lineares sem descarte de picos, clipping e
preenchimento fracionário discreto. Scratch em faixas de 256 células e até
32 linhas por instrumento limitam o custo. Eixos/grade aparecem quando há pelo
menos 18 colunas e 3 linhas. Duas séries são sobrepostas em altura intermediária,
separadas em canais compactos com 1–2 linhas e em pistas com legendas na expansão.
Sobreposição intermediária permanece limitada à única cor de frente da célula.

`terminal/instruments.rs` reúne unidades e barras de capacidade com resolução
de meia célula. CPU expandida acrescenta tabela de núcleos; em máquinas com
muitos núcleos exibe os mais ocupados nos slots disponíveis. RAM e VRAM usam
capacidade mais histórico; disco distingue capacidade de leitura/escrita.

`DashboardApp` retém modelos semânticos em `Arc`, por revisão de coleta e
contexto. `DashboardV2Renderer` retém um único buffer do painel principal,
validado por identidade dos modelos, dimensões, perfil, foco, tema, expansão e
inventário. A cópia de células ainda custa O(área). Redimensionar substitui esse
buffer; não cria um catálogo ilimitado de tamanhos.

`TerminalCapabilities` detecta opções uma vez. O modo moderno padrão é um
no-op; modos 16/256 cores, mono e ASCII adaptam o buffer final, conservando os
valores e a geometria. Nenhum shader, canvas ou biblioteca gráfica foi adicionado.

## Contratos e manutenção

Novos campos públicos opcionais: `NetworkInfo::sample_interval` e
`TemperatureInfo::critical_celsius` (também no inventário térmico). Consumidores
externos que construam esses structs por literal precisam fornecer os campos.
As funções de coleta existentes e construtores de conteúdo foram mantidos,
com alternativas compartilhadas para o caminho de desenho da aplicação.

Os painéis isolados pela expansão ocupam o corpo disponível em qualquer preset,
incluindo Compact. A preparação dos cinco cards usa heap para evitar cópias grandes
de arrays na pilha; o cache permanece por revisão de coleta/contexto. GPU expandida
mostra as leituras atuais das demais placas, enquanto o histórico é identificado
como pertencente à principal. Rede expandida lista as interfaces; o total do card
continua excluindo loopback.

Getters do modelo e o iterador de blocos recebem `&self`, evitando copiar todo
o modelo na stack; cada bloco retornado continua sendo um valor. O iterador
agora empresta o modelo durante a iteração, algo a considerar em consumidores externos.
Os testes cobrem buffers, janelas, disponibilidade, escalas, taxas, clipping,
Unicode, caches, navegação e integração. Capturas reproduzíveis e medições estão
em [Resultados](archive/graphics-overhaul/results.md).


## Paleta semântica — revisão de 22/10/2026

O tema padrão Pulse usa fundo `#080808`, superfície `#101010`, texto e texto secundário `#FFFFFF`. Bordas, foco, seleção e informação são neutros.
Verde `#7BD88F`, amarelo `#E8C65A` e vermelho `#F07575` indicam, respectivamente,
telemetria normal, atenção e estado crítico. Os presets opcionais Catppuccin e
Tokyo Night continuam disponíveis com suas paletas próprias.

CPU, memória e GPU usam a classificação existente do modelo para colorir o
histórico inteiro pelo **estado atual**; isso não classifica individualmente cada
amostra antiga. Valores e marcadores `!`/`×` continuam permitindo leitura sem cor.
As barras de utilização usam os limites existentes de 80% e 95%; capacidade de
disco usa 85% e 95%. Esses limites indicam utilização/pressão, não diagnóstico de
falha física. Temperaturas mantêm os limites conhecidos do sensor quando disponíveis.
Rede e I/O usam branco e verde, com legendas e posições distintas, pois taxa alta
isoladamente não define um estado crítico. Tendência ascendente de load deixou
de receber amarelo por si só.

`Theme::pulse_scene` restaura a paleta original roxa/ciano, incluindo os tons
de alerta e destaques anteriores, exclusivamente para o SYSTEM PULSE. A alteração é exclusivamente de apresentação: os algoritmos de
movimento, energia, iluminação e rasterização não foram modificados. O campo
continua aplicando os destaques de pressão existentes. Seus testes de cor usam a
mesma paleta derivada aplicada pelo dashboard.

Após os overlays e antes do fallback de terminal, `Theme::apply_text_palette`
aplica branco aos caracteres textuais (letras, números e pontuação de unidades)
do tema padrão. Remove DIM desses caracteres; mantém cores de Braille, blocos,
barras e símbolos de estado. O logotipo usa fundo neutro para preservar contraste.
