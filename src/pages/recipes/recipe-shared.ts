export interface RecipeIndex {
  id: string;
  name: string;
  trigger_type: string;
  is_enabled: boolean;
  last_error?: string | null;
  last_run_at?: string | null;
  total_runs: number;
}

export interface RecipeRun {
  id: string;
  recipe_id: string;
  status: string;
  current_step_id?: string | null;
  started_at: string;
  completed_at?: string | null;
  error_message?: string | null;
}

export const DEFAULT_YAML = `id: nova-receita
name: Nova receita
enabled: false
triggers: []
flow:
  - type: step
    id: passo-1
    name: Primeiro passo
    mode: build
    agent: build
    effort: low
    prompt: |
      Execute o primeiro passo e salve o resultado em {{run.output_dir}}.
`;
