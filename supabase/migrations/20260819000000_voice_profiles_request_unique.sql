delete from public.voice_profiles where request_id is null;

drop index if exists voice_profiles_user_request_idx;

alter table public.voice_profiles
  alter column request_id set not null;

alter table public.voice_profiles
  add constraint voice_profiles_user_request_key unique (user_id, request_id);
