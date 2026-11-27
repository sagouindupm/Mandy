# Perfil "NVIDIA Compatibility" e diagnóstico de tela preta (2026-11-25)

Máquina: Windows 11, Ryzen 5 5600X, **RTX 3060 12 GB** (driver 621.40 / UMD 32.0.16.2140),
32 GB de RAM, 24 GB de arquivo de paginação, PowerShell 5.1.

O executável do título **não foi executado, lido nem modificado** neste trabalho. Tudo o que está
abaixo foi validado com programas próprios e com a suíte de testes do VIctory.

## Por que este fork e não o VIctory-7900XTX

Os dois têm a mesma base (o 7900XTX é um fork deste). A diferença é onde cada um foi medido:

| | VIctory-7900XTX | victory-fork (este) |
|---|---|---|
| Testado em | RX 7900 XTX, 64 GB | **RTX 4070 12 GB, 32 GB** |
| Memória em jogo | 40–46 GB privados ("64 GB recomendados") | 23–27 GB |
| Ajustes de `play.ps1` | RDNA 3 | conjunto medido em NVIDIA (`diagnostics/README.md`) |

Com 32 GB de RAM, o 7900XTX já está fora do que o próprio autor recomenda.

## O que a RTX 3060 aceita e recusa (medido)

`victory-gfxcheck` (programa próprio, `E:\GTA6.Xbox.v1.0.2.1\victory-gfxcheck`): **12 de 12 testes
passaram.** Fatos que importam para a camada:

| Fato | Valor medido | Consequência no VIctory |
|---|---|---|
| Ondas (wave lanes) | mín 32, máx 32 | Programas wave64 do console precisam das formas de 32 lanes do tradutor |
| Compute `[WaveSize(64)]` | **recusado**, `E_INVALIDARG` (0x80070057) | Um programa traduzido assim perde seus dispatches: o tradutor escolhe 32 lanes por `set_host_max_wave` |
| Heap de views visível ao shader | 1.000.000 aceito; 2^20, 2^21, 2^22 **recusados** | Sem "raw twins"; `probe_view_limit` limita índices ao slot nulo 999.999 |
| Shader model | 6.8 | Atende (pede-se 6.6) |
| Mesh shaders | tier 1 (DispatchMesh testado com leitura de volta) | Atende |
| Ray tracing | tier 1.2 | Disponível; desligado por padrão |
| Binding / heap tier | 3 / 2 | Atende |
| Atômicos 64 bits em recursos tipados | sim | Atende |
| VRAM | orçamento 11.342 MiB; 4 GiB alocados e devolvidos sem vazamento | Evict/MakeResident funcionam |
| Sincronização | 2.000 idas e voltas CPU↔GPU (1,5 µs); 200 cadeias direct→compute→copy sem deadlock | Sem deadlock nas filas |
| Apresentação | 300 frames com vsync, todos os `Present` = S_OK | SwapChain flip-discard funciona |

Tradução de shaders do próprio VIctory (`cargo test -p victory-shader --features host`):
**217 passaram, 0 falharam, 3 ignorados**. Os testes que executam DXIL traduzido rodaram na RTX 3060
(ex.: `wave64_tests ... ran on the hardware device`). Os testes "diff" que comparam com um
compilador de console foram pulados: esse compilador não existe nesta máquina.

Conclusão: **placa, driver, DirectX 12 e o tradutor funcionam nesta máquina.** A tela preta não
vem de uma incompatibilidade básica da RTX 3060.

## O que se sabe da execução que ficou preta

Não havia nenhum log gravado (o log vai só para o console). O que restou no cache
(`%LOCALAPPDATA%\victory\cache`) da execução das 21:29–21:39 com o 7900XTX:

- 25.718 shaders traduzidos no cache (tradução completa).
- A DLL `amd_fidelityfx_upscaler_xbox` carregada tarde (21:36): o título passou da inicialização.
- Biblioteca de pipelines de **109 KB** e 16 pipelines na lista "early"; 5 delas levaram
  **1,0–1,3 s cada** para o driver compilar. Uma biblioteca completa tem ~3.400 pipelines e 366 MB.

Leitura: o título fez poucas pipelines, a GPU ficou em 3 %, e o driver NVIDIA estava com cache
frio. O próprio fork documenta "frames pretos enquanto as pipelines são refeitas" num cache frio.
Isso é **compatível** com a tela preta, mas **não está provado**: só um log da próxima execução
diz qual etapa parou. Por isso o watchdog abaixo.

## O que foi adicionado

### 1. Watchdog de tela preta — `crates/victory-gpu/src/gfxwatch.rs` (novo)

Registra, uma vez e com os segundos desde a criação do device, cada etapa alcançada:

1. device D3D12 criado · 2. primeiro shader traduzido · 3. primeira pipeline do host criada ·
4. primeira command list do título enviada · 5. primeiro draw · 6. primeiro dispatch ·
7. primeiro Present do título · 8. swap chain criada na janela · 9. primeiro frame mostrado
(Present S_OK) · 10. primeiro frame que não é preto

A cada 5 s, se a janela ficou 10 s sem frame, grava uma linha `BLACK WINDOW` com: última etapa,
etapas não alcançadas, Presents do título / do host / pretos / com falha, último HRESULT, listas,
draws, dispatches, shaders ok/falhos, pipelines ok/recusadas/ainda sendo feitas, VRAM, memória
compartilhada, memória privada do processo, e se o device foi removido — mais um veredito em
palavras (ex.: "o título ainda não apresentou enquanto pipelines são feitas: compilação de
pipelines na primeira execução").

`VICTORY_GFX_WATCH=<segundos>` (padrão 5; `0` desliga). Só lê contadores atômicos: nenhum lock
das threads do título.

Ganchos (uma linha cada): `host.rs` (device pronto, swap chain criada, resultado do Present),
`queue.rs` (Present do título, ExecuteCommandLists), `pipeline.rs` (shader traduzido ou não),
`pipeline/make.rs` (pipeline criada ou recusada), `lib.rs` (módulo).

Testes de regressão: `gfxwatch::tests` (3), todos passando.

### 2. Perfil NVIDIA Compatibility — `scripts/play-nvidia.ps1` (novo)

- Detecta a GPU (Win32_VideoController); recusa rodar sem NVIDIA salvo `-Force`.
- Usa o conjunto de opções medido na RTX 4070 12 GB / 32 GB (`diagnostics/README.md`), todas
  verificadas como existentes no código.
- `VICTORY_D3D12_DRED=faults` (rastreio barato de perda do device) e o watchdog ligados.
- `-Console SeriesS` passa `--console series-s` (o título pede menos memória); padrão Series X,
  que é o testado.
- `-RayTracing` opcional (`VICTORY_BVH_BUILDER=1`), desligado por padrão.
- `-Plain`: só os padrões do build, para isolar problemas.
- Avisa se a memória de commit livre for menor que 30 GB.
- **Grava o log completo** em `%LOCALAPPDATA%\victory\logs\run-<data>.log` (fora da árvore de
  código, como exige `docs/asset-policy.md`), com a GPU, o driver e todas as opções no topo; na
  janela mostra só etapas, watchdog, avisos e erros; ao sair, imprime as últimas linhas de etapa.

O que é específico de NVIDIA já é decidido pelo código a partir do que a placa informa (32 lanes,
heap de 1.000.000, views de constantes de 64 KiB, dispatches indiretos de até 65.535 grupos); o
perfil AMD (`VIctory-7900XTX\scripts\play.ps1`) não foi alterado.

### 3. Testes gráficos independentes — `E:\GTA6.Xbox.v1.0.2.1\victory-gfxcheck` (novo)

Crate próprio (fora das duas árvores do VIctory). Testes: janela; device, adaptador e recursos;
cor sólida pela swap chain com leitura de volta; triângulo; upload e amostragem de textura;
compute com verificação dos 1.024 valores e do número de lanes; mesh shader; criação de pipelines
(32 compute distintas, gráfica, mesh, e WaveSize(64)); sincronização CPU↔GPU e entre filas com
timeout (detecta deadlock); heaps de descriptors nos tamanhos que a camada pede; 300 frames
contínuos com tempos; alocação de até 4 GB de VRAM, residency e devolução. Relatório também em
`%LOCALAPPDATA%\victory\gfxcheck\`.

## Memória com 32 GB de RAM e 12 GB de VRAM

- Medido neste fork, numa máquina igual em RAM/VRAM: 23–27 GB do PC e 9,2–9,6 GB dos 10,3 GB de
  orçamento da placa em jogo, sem estouro de orçamento em nenhuma amostra.
- Aqui: 11,3 GB de orçamento de VRAM, 19 GB de RAM livre e 41 GB de commit livre antes de rodar.
  A folga é pequena: feche navegadores e outros programas.
- O arquivo de paginação evita falhas de commit (erro 1455), mas não substitui RAM: o que for
  paginado fica lento. `-Console SeriesS` é a alavanca para pedir menos memória ao título.

## Primeiras execuções reais (2026-11-25, 22:12 e 22:18)

Execução 1 (cache de shaders do 7900XTX, driver frio):

- Todas as 10 etapas gráficas alcançadas em 5 s; menus a 30 fps; nenhum erro, nenhuma perda do
  device; 22,5 GB de RAM e 9,4 de 11,2 GB de VRAM.
- Na cena "Vice City, dois anos atrás": 800–950 pipelines na fila e 90–920 draws descartados por
  frame ("without a host pipeline: being made"). **Esta é a tela preta com só a legenda**: os draws
  do cenário esperam pipelines que o driver ainda compila; a legenda já tinha a sua.
- `shaders from the cache ... hits=0`: o cache de shaders do 7900XTX não serve a este fork (outro
  tradutor, outra chave). Os 1.122 shaders da execução foram traduzidos durante o jogo.
- Não travou: a janela foi fechada ("title did not finish suspending in time; ending the title").

Correções:

- `victory translate E:\GTA6.Xbox.v1.0.2.1\shaders.rpf --warm all --jobs 12`: 25.636 shaders
  traduzidos para o cache deste fork em 4m51s, 253 não traduzem (geometria e sombras por ray
  tracing, já conhecidos). Na execução 2 o cache passou a acertar (hits=229 contra 0).
- `VICTORY_PIPELINE_WORKERS` (novo neste fork, `pipeline/make.rs`, com teste): o número de workers
  era fixo em metade dos threads (6 numa 5600X). `play-nvidia.ps1 -Warmup` usa threads − 2.

Execução 2 (biblioteca de 825 pipelines da execução 1):

- 653 pipelines carregadas da biblioteca em vez de compiladas; 1.656 começaram cedo, nos menus.
- Ritmo medido: ~570 pipelines/minuto enquanto são leves; depois ~10/minuto com os 6 workers e
  mais 3 threads do driver (prioridade Idle) ocupando a CPU inteira (100 %): as que sobram levam
  ~30 s cada. Não há trava; é compilação, e a CPU de 6 núcleos é o limite.
- Erro meu, já entendido: a tradução de shaders rodando ao mesmo tempo (12 threads, prioridade
  normal) deixou os workers (abaixo do normal) sem CPU de 02:20 a 02:24.

Consequência prática: as primeiras execuções são de aquecimento. Cada uma soma pipelines à
biblioteca (`%LOCALAPPDATA%\victory\cache\pipelines\host-pipelines.bin`), e as seguintes as
carregam em milissegundos. Uma fila ("being made") em 0 é o sinal de que a imagem pode aparecer.

## Execução 3: o mundo aparece (22:42)

Com shaders no cache (2.672 acertos de 2.676) e a biblioteca de pipelines de 234 MB, a fila de
pipelines chegou a 0 e o mundo renderizou: ~6–7 fps, 150 ms por frame, 2.000–3.700 draws, sem
erros. O que restou de draws descartados (200–290 por frame) é do construtor das estruturas de
ray tracing, desligado; por isso tudo o que o sol não ilumina direto fica preto.

## Luz ambiente (portada do VIctory-7900XTX)

`crates/victory-gpu/src/ambient.rs` (de VIctory-7900XTX, ideia de beloooo / VIctory-Belobug-AMD,
GPL-3.0-or-later): logo depois do programa do título que resolve a luz difusa das sondas de tela
(`CS_ScreenProbes_TemporalResolve_*`), eleva a luz a uma estimativa tirada do próprio frame (a luz
que as superfícies visíveis emitem, média em 64x36 células, como harmônicos de 1ª ordem em 160x90
pontos), com sombreamento nas dobras a partir da profundidade. Só age enquanto o construtor de
estruturas de ray tracing está desligado.

- Ganchos: `lib.rs` (módulo), `pipeline.rs` (papel do shader, pelo nome no container),
  `draw.rs` (tabelas refeitas por bind e a chamada depois do dispatch, copiados do 7900XTX).
- `VICTORY_AMBIENT`: um número (fração da estimativa; `play-nvidia.ps1` usa 0.8, como o
  `play.ps1` do 7900XTX), `constant:<nível>`, ou `0` para desligar. `-Ambient` no script.
- Se o layout das tabelas deste fork não for o que a luz espera, ela não faz nada e o log diz
  "no stand-in for the indirect light: other tables" (sem risco para a imagem).
- Testes: `ambient::tests` (3), incluindo a criação dos passes na RTX 3060: todos passam.

Suíte do `victory-gpu` com isso: 201 passaram, 1 falhou (`watch::tests::written_pages_are_reported_to_every_asker`,
que passa 3 de 3 rodando sozinho: estado global de write-watch compartilhado com testes em
paralelo; `watch.rs` não foi tocado).

## Otimização (execução 4 em diante)

Execução 4 (luz ambiente ativa, tudo compilado: shaders 2.699/0, fila de pipelines 0): 0,6–4,4 fps
em cutscene. O frame é CPU copiando a memória do título: atualização de texturas ~219 ms, cópias
180–210 MB, cópia de volta 55–110 ms, a GPU quase ociosa. Compilar mais não muda nada.

O que entrou em `play-nvidia.ps1` (ligado por padrão; `-NoFast` desliga):

| Opção | O que corta (estimativa do autor do fork, não medida em jogo) |
|---|---|
| `VICTORY_FILLS_PLAIN=1` | 8–12 mil preenchimentos pequenos por frame sem chamada ao kernel (~3 ms) |
| `VICTORY_STEP_INDEX=1` | busca de cópias por índice em vez de lock (1,5–4 ms) |
| `VICTORY_LIGHT_REFRESH=1` | refreshes de blocos intocados sem lock nem kernel (cópias 43→30 ms em ambientes fechados, 80→57 fora) |
| `VICTORY_ABSENT_ONCE=1`, `VICTORY_ABSENT_MS=told` | não pergunta ao Windows de novo por blocos já sabidos ausentes (44 ms/frame fora) |
| `VICTORY_REFRESH_ONCE=1` | memória atualizada uma vez por envio |

Os modos `alt` dessas opções têm relógios de 20 s independentes (cada um começa no primeiro uso),
então não alternam juntos: compare duas execuções (normal e `-NoFast`), não uma.

Build `release` (`cargo build --locked --release -p victory`, `-Build release` no script): sem a
feature `diagnostics`, que soma estatísticas por frame. Watchdog e luz ambiente estão nele também;
as linhas `framelog` detalhadas e a detecção de frame preto ficam só no `diag`.

### Medido (execução de 2026-11-26 00:06 contra a de 2026-11-25 22:51)

Cenas diferentes (a segunda com ~2x mais draws), então os números são indicativos:

| CPU por frame | Sem as 5 opções | Com elas |
|---|---|---|
| Atualização de texturas (todas as threads) | 218,9 ms | 10,7 ms |
| Passos das listas (cópias, preenchimentos) | 255,1 ms | 55,6 ms |
| Write-watch e busca sob voos | 68,8 ms | 14,0 ms |
| Flush das atualizações | 52,5 ms | 8,4 ms |
| Gravação, todas as threads | 2.175 ms | 469 ms |

Na mesma execução, a primeira versão de `-MoreTextures` (janelas 1 GiB, buffers 768 MiB na
placa) deixou 1.760 MB de cópias na placa contra 3.126 MB, e o tempo da GPU nas listas do título
foi a 235–287 ms por frame contra 28–53 ms: buffers na RAM do PC lidos pelo barramento. O ganho
de CPU sumiu atrás disso. `-MoreTextures` agora só sobe o teto das texturas para 94 %.

### Execução de 2026-11-26 00:14 (`-MoreTextures` corrigido) e 00:21 (`-Display 1080p`)

- 00:14: mundo com 5.000–6.800 draws a 6,4–6,9 fps (145–156 ms); na execução de 22:42, 2.000–3.700
  draws davam a mesma velocidade. GPU quase sempre ocupada (ociosa 7–18 ms): listas do título
  74–96 ms, listas da camada (hash e comparação das cópias na placa) 36–46 ms. Novo gargalo: GPU.
- 00:21: `VICTORY_DISPLAY_MODE=1920x1080` foi informado ao título (log: "display mode reported to
  the title width=1920 height=1080") e ele **continuou renderizando em 2560x1440**. Sem ganho.
- 00:21: 7,4–7,7 fps com 4.000–6.000 draws, e depois de ~4 min no mundo a placa travou:
  `0x887A0006` (DEVICE_HUNG), DRED "page fault" sem endereço, VRAM em 10,6 de 11,3 GB (teto de
  texturas em 94 %). Hipótese, não provada: orçamento de VRAM no limite, paginação, um comando
  passa de 2 s (TDR padrão do Windows) e o Windows reinicia a placa. `-MoreTextures` deixou de ser
  recomendado em placas de 12 GB.

### Resolução dinâmica do título e verificações na placa (2026-11-26)

- **Por que o título não baixa a resolução:** ele mede o próprio tempo de GPU com quatro
  timestamps por frame, em pares. A camada escreve todos quando as últimas submissões do frame
  terminam, a microssegundos um do outro, então cada par diz "a GPU não fez nada", e a resolução
  dinâmica fica no máximo (2560x1440). `VICTORY_DISPLAY_MODE` não muda isso.
- **Portado do VIctory-7900XTX:** `VICTORY_GPU_LOAD_MS=<ms>` (`queue.rs`, `gpu_load`/`as_loaded`):
  o início de cada par é escrito `ms` antes do fim, e o título "vê" a GPU levar esse tempo. Acima
  do orçamento de 33 ms ele pode baixar a resolução sozinho. Teste
  `queue::tests::a_pair_of_timestamps_shows_the_load_it_is_given`. Script: `-GpuLoadMs 40`.
  O efeito no título não está medido (o autor do 7900XTX também não registrou).
- **Medir:** `ambient.rs` agora diz "the title's render size" a cada mudança do tamanho da
  resolução da luz difusa.
- **`VICTORY_CARD_HASH_WIDE=1`** entrou nas economias padrão: os hashes das cópias na placa com 64
  threads por pedaço de 64 KiB em vez de uma (PR #1014 do fork, medido numa RTX 3060). Aqui as
  listas de hash e comparação da camada eram 36–46 ms da GPU por frame com a GPU como limite.
- Suíte do `victory-gpu`: 202 passaram, 1 falhou (o teste do WARP já conhecido como instável).

Descartado: `VICTORY_INPLACE` (memória do título como heap da GPU) cria o heap na RAM do PC, o
mesmo caminho lento pelo barramento.

## Sobre 60 fps

Não é alcançável com esta camada: o título trava a si mesmo em 30 fps (como no console), e no
mundo aberto este fork mede 7–12 fps numa RTX 4070 com i7-14700KF (28 threads). O limite medido é
a thread que prepara os envios à GPU (CPU), não a placa: a GPU fica ~3 % ocupada nos logs daqui.

## Suíte do `victory-gpu` depois das mudanças

`cargo test --locked --release -p victory-gpu`: **197 passaram, 1 falhou, 5 ignorados.** A falha,
`indirect::tests::warp_budget_saturates_without_wraparound_and_preserves_draw_fields`, é anterior
a este trabalho: roda no adaptador de software (WARP), em `indirect.rs`, que não foi tocado. Este
fork mudou a asserção para exigir que os registros 1 e 2 sejam os mantidos, o que depende da ordem
em que as threads fazem o `InterlockedAdd` do orçamento. A versão do mesmo teste no
VIctory-7900XTX, que não tem essa exigência, passa. Não tem relação com a tela preta.

## Limites (o que NÃO foi validado)

- A tela preta em si: o título não foi executado aqui. A causa só é confirmada pelo log da próxima
  execução (linhas `graphics stage reached` e `BLACK WINDOW`).
- O watchdog dentro de uma execução real do título (testado só em unidade).
- Desempenho e estabilidade em jogo na RTX 3060.
- A suíte completa do `victory-shader` com comparação contra o compilador do console (ausente).

## Comandos

```powershell
cd E:\GTA6.Xbox.v1.0.2.1\victory-gfxcheck
.\target\release\victory-gfxcheck.exe
```

```powershell
cd E:\GTA6.Xbox.v1.0.2.1\victory-fork-main\victory-fork-main
cargo build --locked --profile diag -p victory --features diagnostics
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts\play-nvidia.ps1 -Exe E:\GTA6.Xbox.v1.0.2.1\game_scarlett_master.exe
```

## Frame pacing (1% low, P95/P99, travadas)

Novo módulo `crates/victory-gpu/src/pacing.rs`. Cada Present do host que dá certo grava o tempo desde o anterior. A cada 10 s o log traz uma linha `frame pacing, last 10 s` com:

- o fps médio, o 1% low e o 0,1% low;
- a mediana, o P95, o P99 e o maior tempo de frame;
- quantos frames passaram de 100 ms, 250 ms e 1 s, na janela e desde o início.

O script já mostra essa linha no terminal. `VICTORY_PACING=0` desliga. Há 3 testes (frames iguais; uma travada de 2 s; janela vazia).

## Medições da rodada de 26/11 07:49 (-GpuLoadMs 40, CARD_HASH_WIDE=1)

| | antes (00:14) | agora |
|---|---|---|
| fps no mundo | 6,4–7,7 | 7,7–9,1 |
| listas de envio+hash e de comparação na placa | 36–46 ms | 11–14 ms |
| listas do jogo na placa | 74–96 ms | 62–92 ms |
| thread que submete, trabalhando por frame | – | 86–101 ms |

- A resolução não mudou com 40 ms: continua 2560x1440. O próximo teste é 100 ms.
- A placa fica ocupada ~84% do frame. Ela e a thread que submete levam ~90 ms cada, então as duas limitam juntas.
  - Baixar a resolução aliviaria a placa e também diminuiria a cópia de volta (50–70 MB por frame).
  - Do lado da CPU, o maior item é o espelhamento do heap de descritores: 19,6 ms por frame, dos quais 14,2 ms criam os slots do host para ~12.600 registros novos.

## Travadas de 1–6 s: remapeamento da memória do jogo (rodada de 26/11 08:13)

O módulo `crates/victory-hle/src/modules/xmem.rs` ganhou uma linha a cada 5 s, `title memory remapping in the last ... s`, que separa o tempo do remapeamento em: chamadas ao Windows, commit, avisos à GPU e trechos remapeados.

**O que a rodada mostrou:**
- Fora das travadas, o remapeamento custa ~5–46 ms a cada 5 s.
- Nas rajadas de travada, o jogo faz 7.000–11.400 chamadas em 5 s, que somam **4,3–4,9 s**.
  - Quase tudo é chamada ao Windows (`VirtualFree` para dividir placeholders, `MapViewOfFile3`, `UnmapViewOfFile2`): uns 0,5 ms cada.
  - O commit custa só 2–21 ms e os avisos à GPU 70–210 ms.
- Os maiores frames somam ~1,0–1,3 s de remapeamento cada.

**Correção 1, `VICTORY_REMAP_SAME`** (ligada por padrão; `=0` volta ao comportamento antigo):
- Um remapeamento que pede exatamente o que a faixa já mostra (mesma seção, mesma página, endereço por endereço) agora é deixado como está.
- Isso vale também para desmapear o que já está desmapeado e para a cópia de mapeamento de páginas da GPU (`map_alias`).
- O conteúdo visível é o mesmo, então não há nada que a GPU ou uma leitura no lugar possam ver diferente.
- A linha do log conta essas chamadas.
- Teste novo: `mapping_what_is_there_already_leaves_the_views_alone`. Todos os 103 testes do victory-hle passam.

Se poucas chamadas forem desse tipo, o próximo passo é atacar o custo das próprias chamadas ao Windows. A linha agora separa o tempo de unmap do de split+map.

### Resultado da correção 1 (rodada de 26/11 09:47) e o que veio depois

- Ela quase não atua: de cada ~3.500–12.600 chamadas em 5 s, só 0 a 4 pediam o que já estava lá. A sessão teve 4.489 frames, 509 acima de 100 ms, 150 acima de 250 ms, 30 acima de 1 s, e a maior travada durou 6,6 s. É o mesmo quadro de antes.
- O custo está nas chamadas ao Windows: ~1 ms por view mapeada (4.440 views em 4,8 s, por exemplo).
- Teste isolado (programa próprio, fora do jogo): repete o que `map`/`unmap` pedem ao Windows, ou seja, views de 64 KiB de uma seção SEC_RESERVE de 6 GiB num placeholder de 8 GiB.

  | condição | por remapeamento | pior map |
  |---|---|---|
  | 5.000 views | 8 µs | – |
  | 100.000 views | 9 µs | – |
  | 2 threads de VirtualQuery | 12 µs | – |
  | 2 threads de VirtualProtect | 16 µs | 1,0 ms |
  | 1 thread de GetWriteWatch em 1 GiB | 17 µs | 5,3 ms |

  Ou seja, as chamadas são baratas. No jogo elas custam ~100 vezes mais, o que aponta para espera pelo espaço de endereços do processo (as guardas com VirtualProtect, o GetWriteWatch do lado da GPU, as falhas de página) ou por um núcleo livre.
- A linha `title memory remapping` agora mostra quanto tempo a thread rodou nessas chamadas (`QueryThreadCycleTime`) contra o tempo de relógio, e separa o tempo de split de placeholder. Com isso a próxima rodada diz qual dos dois é a causa.
- Nas áreas pesadas, os 6 fps estáveis não vêm do remapeamento, que ali custa só 8,6 ms por frame. No frame típico de 156 ms:
  - a thread que submete trabalha 110 ms (os passos das listas 54, o write-watch 15, card_before 10, as atualizações 9);
  - a cópia de volta leva 42 ms (48 MB) e as cópias 39 ms (110 MB);
  - a gravação das chamadas de lista do jogo soma 483 ms de CPU em 51.641 chamadas (~9 µs cada).

### Build de desempenho (`perf`)

`scripts/build-perf.ps1 -Cpu znver3` gera `target\perf\victory.exe`:
- release sem a feature `diagnostics`;
- LTO fat e uma unidade de geração de código (`[profile.perf]` no Cargo.toml);
- código para o Zen 3.

Para jogar com ele: `scripts\play-nvidia.ps1 -Build perf -Exe ...`. A linha de frame pacing, a de remapeamento e o framelog continuam no log, porque não dependem de `diagnostics`.
