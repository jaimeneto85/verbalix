create extension if not exists pg_cron with schema extensions;

create table public.transform_history (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users(id) on delete cascade,
  operation text not null check (operation in ('translate', 'improve')),
  source_text text not null check (char_length(source_text) between 1 and 12000),
  result_text text not null check (char_length(result_text) > 0),
  source_language text not null,
  target_language text,
  preferences jsonb,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '30 days'
);

create index transform_history_owner_created_idx
  on public.transform_history (user_id, created_at desc);

alter table public.transform_history enable row level security;

create policy "owners can read history"
  on public.transform_history for select
  to authenticated
  using ((select auth.uid()) = user_id);

create policy "owners can create history"
  on public.transform_history for insert
  to authenticated
  with check ((select auth.uid()) = user_id);

create policy "owners can delete history"
  on public.transform_history for delete
  to authenticated
  using ((select auth.uid()) = user_id);

create or replace function public.delete_expired_transform_history()
returns void
language sql
security definer
set search_path = ''
as $$
  delete from public.transform_history where expires_at <= now();
$$;

revoke all on function public.delete_expired_transform_history() from public;

select cron.schedule(
  'delete-expired-transform-history',
  '17 3 * * *',
  $$select public.delete_expired_transform_history()$$
);
