# Auditoria da telemetria — 22/10/2026

Revisão do coletor, adaptação, modelos e desenho da versão baseada em `3b24a57`.
Foram conferidos os valores disponíveis na máquina e casos sintéticos controlados,
incluindo foco, expansão, nova coleta e retorno ao dashboard.

## Correções implementadas

- CPU global e por núcleo aguardam um intervalo válido; a primeira leitura mostra
  `—`, sem transformar uma média desde o boot em atividade instantânea.
- CPU por processo verifica criação do processo além do PID. Falta de baseline,
  reset ou tempo nulo são indisponíveis, em vez de zero ou uma taxa inventada.
- Discos preservam identidades como `dm-0` e `md127`. Aliases de `/dev` são resolvidos
  antes da deduplicação, sem somar a capacidade do mesmo volume várias vezes.
- I/O do dispositivo no Linux vem de deltas dos seus contadores em sysfs, incluindo
  atividade fora das partições montadas. Startup, hotplug e reset não criam picos.
  Capacidade continua representando os sistemas de arquivos montados, não todo o
  tamanho bruto de um disco nem espaço não particionado.
- UUID da NVIDIA e endereço PCI do inventário DRM preservam identidade entre probes;
  duas GPUs iguais não trocam de histórico só por mudar a ordem da resposta.
- Utilização indisponível da NVIDIA não descarta temperatura, VRAM e clock válidos.
  VRAM inconsistente ou armazenamento com total zero não bloqueiam outros modelos.
- Temperaturas negativas fisicamente possíveis são preservadas. Núcleo indisponível
  não é convertido silenciosamente em 0%.
- Resumo, processos e temperaturas expandidos deixavam de aparecer em alguns
  presets compactos. Um painel isolado agora recebe o corpo disponível.
- O cabeçalho de foco térmico agora indica TEMPERATURES, em vez do nome legado HISTORY.
- CPU expandida em 80×24 usa o espaço para frequência, temperatura e núcleos.
  GPU mostra potência quando informada e leituras das placas adicionais; o histórico
  permanece identificado com a principal. Rede mostra os valores por interface.
- A preparação dos cards foi movida para o heap. A auditoria reproduziu um estouro
  de pilha no desenho em debug; a correção passou sem aumentar a pilha dos testes.

## Conferência por componente

| Componente | Fonte e significado | Conferência realizada |
| --- | --- | --- |
| CPU | `sysinfo`: uso global, uso por CPU lógico e frequência média em Hz | Comparação com deltas de `/proc/stat`; campos e unidades em foco/expansão |
| RAM | `MemTotal - MemAvailable`, em GiB; cache recuperável permanece disponível | Total e disponível comparados com `/proc/meminfo`; valores, percentual e swap nos modos de tela |
| Swap | Total e ocupado, separados de RAM | Comparação com `SwapTotal - SwapFree`; conversão e exibição |
| GPU NVIDIA | `nvidia-smi`: %, MiB convertidos para bytes, MHz para Hz, °C e W opcionais | Query real com métricas disponíveis; parsing de N/A, unidades, VRAM inválida, IDs e múltiplas GPUs |
| Outras GPUs | Inventário DRM, com campos ausentes quando não coletados | GPU AMD detectada nesta máquina sem métricas pelo coletor atual; fixture valida valores de uma segunda GPU |
| Rede | Delta de bytes / tempo monotônico; soma exclui loopback | `/proc/net/dev`, unidades binárias, agregação e interfaces individuais; startup/reset nos testes |
| Armazenamento | Capacidade dos volumes montados; taxas do dispositivo identificado | `df -B1` e `/proc/diskstats`; unidades, deduplicação, IDs, primeira amostra e reset |
| Temperatura | Sensores do `sysinfo`; limite crítico informado pelo driver | Três sensores presentes; valores e limite de referência em captura expandida; dados negativos e ausentes |
| Processos | Memória em MiB no coletor; CPU relativa a um núcleo | Intervalo próprio, reset/PID, ordem da apresentação, PID/nome/CPU/memória no painel expandido |
| Resumo | Contagem de processos, cores/threads, load e uptime | Consistência com snapshot e `/proc/uptime`; valores e atualização no modo expandido |

CPU de processo pode ultrapassar 100% quando utiliza vários núcleos. O load average
é uma quantidade de tarefas, não percentual de CPU; sua barra é relativa aos threads
lógicos. Rede alta e I/O alto não constituem, por si só, falha de hardware.

## Evidências

O [registro da execução real](validation/telemetry/live-check.txt) contém três
amostras espaçadas em aproximadamente 2,1 s:

- CPU apresentou diferença inferior a 0,05 ponto percentual em relação ao procfs.
- Total de RAM e swap coincidiram; disponível variou até 1.900.544 bytes entre
  leituras assíncronas. A identidade usada + disponível = total foi preservada.
- As taxas de rede coincidiram nas amostras observadas; foram conferidas quatro
  direções de I/O, correspondentes a dois dispositivos, e nove entradas de capacidade.
- A NVIDIA informou utilização, 546 MiB de VRAM ocupada, 8.188 MiB totais, 49°C
  e clock de 210 MHz naquele momento. Potência veio indisponível e permaneceu ausente.

Baseline: 326 testes passando. Após as correções: **329 testes unitários e dois
testes de integração passaram**, sem testes ignorados. Os dois testes de integração
cobrem 40 cenários (oito painéis × cinco tamanhos), com foco, expansão, atualização
e retorno, além de dados inválidos e vários dispositivos. Não foram adicionados
testes de cor ou layout que apenas repetem a implementação.

Build release, Clippy com `-D warnings`, formatação e smoke real em Unicode/True Color
e ASCII/mono passaram. O [registro do smoke](validation/telemetry/terminal-smoke.jsonl)
confirma saída 0 e restauração de tela alternativa e modo raw.

Foram geradas 57 capturas com `--all-focus`. Inspeção visual incluiu
[CPU expandida em 80×24](validation/telemetry/cpu-expanded-80x24.png),
[temperaturas em 80×24](validation/telemetry/temperatures-expanded-80x24.png)
e GPU expandida em 120×35. As imagens usam **dados sintéticos**, não os valores da
execução real. Comandos de reprodução estão em [Desenvolvimento](development.md).

## Limites da verificação

As comparações são assíncronas e usam tolerâncias explícitas no exemplo
`telemetry_check`; não são uma calibração física dos sensores. A query NVIDIA
independente foi feita em outro instante, portanto utilização e VRAM podem variar.
Frequência foi conferida quanto à fonte, média e unidade; não comparada com um
instrumento externo. Potência indisponível não pôde ser validada em hardware.

Processos e disco atualizam a cada dois segundos; GPU é consultada a cada cinco.
Entre essas consultas, a UI apresenta a última leitura disponível. Timestamps do
histórico são do snapshot da aplicação, não um registro de cada chamada do driver.
Históricos de GPU e disco pertencem ao dispositivo principal; os demais mostram
valores atuais. A auditoria não implementou históricos separados por dispositivo.

AMD/Intel sem métricas, RAID/LVM e hotplug foram exercitados por contratos e casos
controlados; não foram todos reproduzidos fisicamente nesta máquina. Os limites
gerais de saúde de CPU/GPU continuam heurísticos; as réguas térmicas usam o limite
do sensor quando disponível. O motor, animação e paleta do SYSTEM PULSE foram mantidos.
