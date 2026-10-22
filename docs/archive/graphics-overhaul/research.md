# Pesquisa dos instrumentos gráficos

Investigação realizada em 12–13/10/2026. Implementação original do SysMon;
nenhum código ou identidade visual de terceiros foi transferido. A licença MIT
do projeto e suas dependências de runtime permanecem iguais.

## Fontes confirmadas

### btop 1.4.7

Instalação local: `/usr/bin/btop`, pacote Arch/CachyOS `1.4.7-1.1`, suporte de
GPU compilado, licença Apache-2.0. Verificado por `btop --version`, metadados e
lista de arquivos do pacote. O executável e as configurações não foram alterados.

O código oficial da mesma versão foi inspecionado:

- [Draw::Graph e medidores](https://github.com/aristocratos/btop/blob/v1.4.7/src/btop_draw.cpp):
  gráficos Braille combinam valores dentro da célula; linhas e medidores têm
  caches; o tamanho disponível limita a resolução. CPU por núcleo e GPU têm
  apresentações específicas.
- [Coletor Linux](https://github.com/aristocratos/btop/blob/v1.4.7/src/linux/btop_collect.cpp):
  rede distingue contadores acumulados e taxas; escalas dinâmicas dependem de
  observações consecutivas, com tratamento de mudanças de interface.
- [Licença](https://github.com/aristocratos/btop/blob/v1.4.7/LICENSE).

Aplicação dos conceitos: conservar o rasterizador Braille do SysMon, separar
capacidade e atividade, compartilhar escala entre direções, evitar alterações
de escala por frame e reutilizar pixels de instrumentos sem novas amostras.
Os parâmetros de histerese e a janela com timestamps são decisões próprias.

Não houve necessidade técnica de REA/Ghidra: o código público respondeu às
questões de renderização e coleta. Nenhuma evidência de engenharia reversa é
alegada.

### Grafana: séries temporais

A [documentação oficial de Time series](https://grafana.com/docs/grafana/latest/visualizations/panels-visualizations/visualizations/time-series/)
expõe limites de eixos, interpolação linear e políticas explícitas para lacunas.
Esses controles ajudam a distinguir dados ausentes de uma série contínua e
variações pequenas de grandes mudanças. No SysMon, isso resulta em percentuais
com domínio conhecido, lacunas preservadas e linhas sem suavização dos valores.
Não foram adotadas bibliotecas web nem a aparência de seus painéis.

### Stack existente

As APIs locais de ratatui 0.30.2 e sysinfo 0.39.5 foram consultadas. `Buffer`
permite inspecionar células e testar clipping sem terminal gráfico;
`Component::critical()` fornece um limite opcional do sensor. Memória usada e
disponível continuam vindo das APIs próprias de sysinfo, sem somar cache a uso
indisponível. O coletor não fornece uma série separada de cache/PSI.

## Auditoria do SysMon em `334fb1f`

| Impacto | Evidência no código/execução | Resolução |
| --- | --- | --- |
| Alto | Apresentação perdia timestamps; largura alterava a quantidade temporal exibida | Timestamps relativos monotônicos, janela de 120 s |
| Alto | Uma amostra preenchia toda a largura | Um ponto real; nenhuma inicialização fictícia |
| Alto | CPU/GPU com autozoom implícito | Domínio fixo 0–100%, eixos quando cabem |
| Alto | I/O tinha histórico coletado, sem desenho no card; escrita omitida em perfis | Traços de leitura/escrita e bloco terciário preservado |
| Médio | Seta de CPU discordava do delta; intervalo inferido da redução de dados | Direção e intervalo da comparação real, unidade pp |
| Médio | Bytes binários rotulados como GB/MB/KB | GiB/MiB/KiB explícitos |
| Médio | Temperaturas sem referência específica | Limite do driver quando disponível; neutro quando desconhecido |
| Médio | Modelos/células refeitos entre coletas | Compartilhamento e cache com invalidação |

Durante a ampliação do histórico, foi observado um stack overflow causado por
cópias grandes de modelos. Corrigido por empréstimo dos getters e modelos
compartilhados; nenhum limite artificial de stack foi aumentado.

## Inferências e limites

Uma grade discreta e rótulos compartilhados foram escolhidos para facilitar a
comparação e combinar com os temas existentes. A avaliação visual é engenharia
e inspeção das capturas, sem estudo de usuários. Não foram investigados outros
monitores indiscriminadamente; as duas referências selecionadas cobriram os
problemas concretos. Compatibilidade com todos os emuladores/fontes não foi
verificada.
