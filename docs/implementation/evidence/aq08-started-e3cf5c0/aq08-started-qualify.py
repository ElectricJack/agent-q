import pathlib,subprocess,os,json,hashlib,tarfile
repo='/tmp/aq08-started'; rev=subprocess.check_output(['git','-C',repo,'rev-parse','HEAD'],text=True).strip();source=pathlib.Path('/tmp/aq08-started-clean');subprocess.run(['git','-C',repo,'worktree','add','--detach',str(source),rev],check=True)
target=pathlib.Path('/tmp/aq08-started-package-target');env=dict(os.environ,CARGO_TARGET_DIR=str(target),CARGO_INCREMENTAL='0')
def run(args,where,name):
 with open('/tmp/aq08-started-clean-'+name+'.log','w') as log:subprocess.run(args,cwd=where,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
run(['cargo','package','-p','aq-admission','--exclude-lockfile','--no-verify','--offline'],source,'package')
archive=target/'package/aq-admission-0.1.3.crate'; artifacts=[(archive,None),(pathlib.Path('/tmp/con03-producer-target/package/aq-lifecycle-protocol-0.1.0.crate'),'945b03a99701035eb7cf6679bd9baa9ad2b50a7cb6329b9ca8de2cf8bd58b92b'),(pathlib.Path('/home/jkern/dev/.aq-qualification/session-host-2026-10-31/aq-session-protocol-0.1.0.crate'),'479778d8ea76f0605b9a20460e0bd3fda73b699337b1e80783ada0d291a68090')]
extracted=pathlib.Path('/tmp/aq08-started-extracted');extracted.mkdir()
for path,sha in artifacts:
 assert sha is None or hashlib.sha256(path.read_bytes()).hexdigest()==sha
 with tarfile.open(path) as tar:tar.extractall(extracted,filter='data')
(extracted/'Cargo.toml').write_text('[workspace]\nresolver="3"\nmembers=["aq-admission-0.1.3","aq-lifecycle-protocol-0.1.0","aq-session-protocol-0.1.0"]\n[patch.crates-io]\naq-lifecycle-protocol={path="aq-lifecycle-protocol-0.1.0"}\naq-session-protocol={path="aq-session-protocol-0.1.0"}\n')
env['CARGO_TARGET_DIR']='/tmp/aq08-started-extracted-target'
for args,name in [(['cargo','test','-p','aq-admission','--offline'],'tests'),(['cargo','clippy','-p','aq-admission','--all-targets','--offline','--','-D','warnings'],'clippy'),(['cargo','fmt','-p','aq-admission','--','--check'],'format')]:run(args,extracted,name)
result={'source_revision':rev,'archive':str(archive),'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'standalone_root':str(extracted),'dependencies':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p,_ in artifacts[1:]},'qualification':'Pure tests, strict all-target clippy, format and publication drift; independent Python canonical digest vectors','limitations':['SQL/HTTP and filesystem handshake are separate qualification; package grants no source/execution authority.']};pathlib.Path('/tmp/aq08-started-clean-qualification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
