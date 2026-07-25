create table public.user_preferences (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null unique references auth.users(id) on delete cascade,
  formality smallint not null check (formality between 1 and 5),
  length text not null check (length in ('concise', 'balanced', 'detailed')),
  tone text not null check (tone in ('neutral', 'friendly', 'assertive', 'technical')),
  history_enabled boolean not null,
  updated_at timestamptz not null default now()
);

alter table public.user_preferences enable row level security;

create policy "owners can read preferences"
  on public.user_preferences for select
  to authenticated
  using ((select auth.uid()) = user_id);

create policy "owners can create preferences"
  on public.user_preferences for insert
  to authenticated
  with check ((select auth.uid()) = user_id);

create policy "owners can update preferences"
  on public.user_preferences for update
  to authenticated
  using ((select auth.uid()) = user_id)
  with check ((select auth.uid()) = user_id);

create or replace function public.set_updated_at()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  new.updated_at := now();
  return new;
end;
$$;

create trigger user_preferences_set_updated_at
  before insert or update on public.user_preferences
  for each row execute function public.set_updated_at();
