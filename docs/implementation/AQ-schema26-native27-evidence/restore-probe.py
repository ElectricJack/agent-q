"""Disposable PostgreSQL17 backup/restore qualification; never a deployment tool."""
from pathlib import Path
import hashlib,json,re,subprocess,uuid
CONTAINER='agent-q-test-db'
IMAGE='sha256:99445113a4a19e82dd3713c65bd9f6fb5bf972065ab043df8d1e1c93e957da9a'
root=Path.cwd()
assert subprocess.check_output(['docker','inspect','-f','{{.Image}}',CONTAINER],text=True).strip()==IMAGE,'unexpected disposable image'
base=['docker','exec','-i',CONTAINER]
created=[]
def sql(database,statement):
 result=subprocess.run(base+['psql','-X','-U','postgres','-d',database,'-At','-v','ON_ERROR_STOP=1'],input=statement.encode(),capture_output=True,timeout=30)
 if result.returncode:raise RuntimeError(result.stderr.decode())
 return result.stdout.decode().strip()
def create():
 name='aq_restore_qualification_'+uuid.uuid4().hex
 sql('postgres','CREATE DATABASE '+name);created.append(name);return name
files=re.findall(r'include_str!\("\.\./migrations/([^"\n]+)"\)',(root/'crates/aq-server/src/store.rs').read_text());assert len(files)==27
try:
 original=create();restored=create()
 initial='BEGIN;CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,applied_at TIMESTAMPTZ NOT NULL DEFAULT now());\n'
 for n,name in enumerate(files[:25],1):initial+=(root/'crates/aq-server/migrations'/name).read_text()+f'\nINSERT INTO schema_migrations(version)VALUES({n});\n'
 initial+="INSERT INTO users(id,email,role,issuer,subject,created_at)VALUES('00000000-0000-0000-0000-000000000001','restore-fixture@example.invalid','owner','fixture','backup-restore',now());COMMIT;"
 sql(original,initial)
 snapshot="SELECT jsonb_build_object('format',(SELECT value FROM meta WHERE key='state_format_version'),'migrations',(SELECT jsonb_agg(to_jsonb(m)ORDER BY version)FROM schema_migrations m),'users',(SELECT jsonb_agg(to_jsonb(u)ORDER BY id)FROM users u),'catalog_profile',EXISTS(SELECT 1 FROM information_schema.columns WHERE table_name='playbook_namespaces'AND column_name='catalog_profile'))"
 before=json.loads(sql(original,snapshot));assert before['format']=='25' and not before['catalog_profile']
 dump=subprocess.run(base+['pg_dump','-U','postgres','--format=custom','--no-owner','--no-acl',original],capture_output=True,timeout=30,check=True).stdout
 assert len(dump)<=8*1024*1024,'synthetic backup byte budget'
 archive=Path('/tmp/schema26-27-restore.dump');archive.write_bytes(dump)
 upgrade='BEGIN;'
 for n,name in enumerate(files[25:],26):upgrade+=(root/'crates/aq-server/migrations'/name).read_text()+f'\nINSERT INTO schema_migrations(version)VALUES({n});\n'
 sql(original,upgrade+'COMMIT;')
 upgraded=json.loads(sql(original,snapshot));assert upgraded['format']=='27' and upgraded['catalog_profile'];assert upgraded['users']==before['users']
 # Restore into a NEW disposable DB. No in-place downgrade or production DB.
 restore=subprocess.run(base+['pg_restore','-U','postgres','--dbname',restored,'--no-owner','--no-acl','--exit-on-error'],input=dump,capture_output=True,timeout=30)
 if restore.returncode:raise RuntimeError(restore.stderr.decode())
 after=json.loads(sql(restored,snapshot));assert after==before,'restore did not preserve exact25 snapshot'
 report={'scope':'Actual pg_dump/pg_restore on two new disposable databases; not an automatic updater/deployment test. Old-reader refusal separately uses same production guard with frozen25 SQL profile.','postgres_image':IMAGE,'backup_bytes':len(dump),'backup_sha256':hashlib.sha256(dump).hexdigest(),'before':before,'upgraded':upgraded,'restored':after,'migration_sha256':{name:hashlib.sha256((root/'crates/aq-server/migrations'/name).read_bytes()).hexdigest()for name in files}}
 Path('/tmp/schema26-27-restore-probe.json').write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps({'original':'25','upgraded':'27','restored':'25','exact_prior_snapshot':True,'backup_bytes':len(dump),'backup_sha256':report['backup_sha256']}))
finally:
 for name in reversed(created):sql('postgres','DROP DATABASE '+name+' WITH(FORCE)')
