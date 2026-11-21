<script lang="ts">
  let { state }: { state: string } = $props();

  const labels: Record<string, string> = {
    ready: "Pronto",
    installing: "Instalando",
    starting: "Iniciando",
    running: "Rodando",
    stopping: "Parando",
    stopped: "Parado",
    error: "Erro",
    interrupted: "Interrompido",
  };
</script>

<span class="status-badge {state}">
  <span class="status-dot"></span>
  {labels[state] ?? state}
</span>
