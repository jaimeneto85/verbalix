# Verbalix Brand and Bundle Startup Fix

## 🎯 SCOPE

### Incluído
- Integrar os assets aprovados da marca na UI e no bundle Tauri.
- Substituir o PNG 16-bit incompatível por ícones 8-bit RGBA válidos.
- Recriar e validar `Verbalix.app`, incluindo `Contents/Resources`, assinatura ad-hoc e execução sem panic.
- Preservar o comportamento funcional já aprovado do MVP.

### Fora do Escopo
- Redesenhar a marca aprovada, alterar fluxos do produto ou distribuir/notarizar o aplicativo.
- Corrigir gates manuais AX da matriz de aplicativos.

### Riscos
- Um PNG visualmente válido pode continuar incompatível com o decoder de ícone se tiver profundidade/canais incorretos.
- Um bundle antigo pode mascarar a correção se não for reconstruído.
- A ausência de resources ou assinatura inválida pode impedir abertura mesmo após eliminar o panic.

## 📋 REQUIREMENTS

- [ ] RF01: Usar mark, wordmark e paleta aprovados sem recriar os assets.
- [ ] RF02: Configurar icon set Tauri com PNGs 8-bit RGBA e `icon.icns`.
- [ ] RF03: Gerar `Verbalix.app` com `Contents/Resources` e ícone empacotado.
- [ ] RF04: O executável deve iniciar sem panic em `did_finish_launching`.
- [ ] RF05: `codesign --verify --deep --strict` deve passar após o build.
- [ ] RF06: Testes, build frontend, Clippy e suíte Rust devem permanecer verdes.

### Critérios de Aceitação
- [ ] CA01: `file` confirma icon master 1024 RGBA 8-bit, icon.png 512 RGBA 8-bit e ICNS válido.
- [ ] CA02: O bundle novo contém executable, Info.plist e Resources/icon.icns.
- [ ] CA03: Abertura real mantém o processo vivo sem crash report ou panic imediato.
- [ ] CA04: UI exibe a marca aprovada e continua responsiva.

## 🏗️ DESIGN

- Assets fonte vivem em `branding/`; outputs de empacotamento vivem em `src-tauri/icons/`.
- `tauri.conf.json` referencia explicitamente o icon set necessário ao target macOS.
- A UI importa o SVG da marca como asset estático e aplica tokens documentados em `BRAND.md`.
- Validação do bundle é feita sobre artefato recém-construído, nunca sobre `target` antigo.

## 📝 TASKS

- [x] T1: Registrar e validar os assets aprovados.
- [x] T2: Integrar mark/paleta na UI sem regressão de layout.
- [x] T3: Reconstruir bundle debug e validar Resources, plist e assinatura.
- [x] T4: Executar launch smoke e confirmar ausência do panic do ícone.
- [x] T5: Rodar suítes e checks de regressão.
- [x] T6: Atualizar memória e documento de entrega.

## Análise Dual Proporcional

### Riscos pessimistas
- Validar profundidade de cor de cada ícone, remover dependência de artefato antigo e checar assinatura/resources separadamente.
- Não considerar “processo abriu” suficiente: confirmar que permanece vivo e não produz panic.

### Oportunidades otimistas
- O diagnóstico já está isolado no icon decoder e os assets novos atendem ao contrato 8-bit RGBA.
- A integração é localizada em assets, configuração Tauri e tokens visuais; o domínio Rust permanece intacto.
