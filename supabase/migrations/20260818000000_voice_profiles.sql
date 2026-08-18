create table public.voice_profiles (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users(id) on delete cascade,
  provider text not null default 'elevenlabs',
  provider_voice_id text,
  display_name text not null,
  status text not null check (status in ('enrolling', 'ready', 'failed', 'deleting')),
  request_id uuid,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create unique index voice_profiles_user_request_idx
  on public.voice_profiles (user_id, request_id)
  where request_id is not null;

create trigger voice_profiles_set_updated_at
  before insert or update on public.voice_profiles
  for each row execute function public.set_updated_at();

alter table public.voice_profiles enable row level security;

create policy "owners can read voice profiles"
  on public.voice_profiles for select
  to authenticated
  using ((select auth.uid()) = user_id);

create policy "owners can create voice profiles"
  on public.voice_profiles for insert
  to authenticated
  with check ((select auth.uid()) = user_id);

create policy "owners can update voice profiles"
  on public.voice_profiles for update
  to authenticated
  using ((select auth.uid()) = user_id)
  with check ((select auth.uid()) = user_id);

create policy "owners can delete voice profiles"
  on public.voice_profiles for delete
  to authenticated
  using ((select auth.uid()) = user_id);
