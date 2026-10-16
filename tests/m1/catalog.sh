#!/bin/sh
set -eu
psql -U m1 -d "$M1_DATABASE" -X -q -At -v ON_ERROR_STOP=1 <<'SQL'
BEGIN READ ONLY;
SELECT json_build_object(
 'applicationRole',(SELECT json_build_object('superuser',rolsuper,'bypassRls',rolbypassrls) FROM pg_roles WHERE rolname='m1_app'),
 'retainedNullChildren',(SELECT count(*) FROM m1_children WHERE tenant_id IS NULL),
 'policies',(SELECT json_agg(json_build_object('table',tablename,'using',qual,'check',with_check)) FROM pg_policies WHERE tablename LIKE 'm1_%'),
 'delegationColumn',(SELECT count(*) FROM information_schema.columns WHERE table_name='jobs' AND column_name='delegated_user_id')
);
COMMIT;
SQL
