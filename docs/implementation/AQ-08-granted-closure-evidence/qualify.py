import pathlib,subprocess,os,json,hashlib,tarfile
source=pathlib.Path('/tmp/aq-granted-closure-contract')
revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=source,text=True).strip()
target=pathlib.Path('/tmp/granted-closure-package-target')
env=dict(os.environ,CARGO_TARGET_DIR=str(target),CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='2')
def run(args,where,name):
    with open('/tmp/granted-closure-portable-'+name+'.log','w') as log:
        subprocess.run(args,cwd=where,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
run(['cargo','package','-p','aq-admission','--exclude-lockfile','--no-verify','--offline'],source,'package')
archive=target/'package/aq-admission-0.1.8.crate'
artifacts=[(archive,None),(pathlib.Path('/tmp/con03-producer-target/package/aq-lifecycle-protocol-0.1.0.crate'),'945b03a99701035eb7cf6679bd9baa9ad2b50a7cb6329b9ca8de2cf8bd58b92b'),(pathlib.Path('/home/jkern/dev/.aq-qualification/session-host-2026-10-31/aq-session-protocol-0.1.0.crate'),'479778d8ea76f0605b9a20460e0bd3fda73b699337b1e80783ada0d291a68090')]
extracted=pathlib.Path('/tmp/granted-closure-extracted');extracted.mkdir()
for path,sha in artifacts:
    assert sha is None or hashlib.sha256(path.read_bytes()).hexdigest()==sha
    with tarfile.open(path) as tar:tar.extractall(extracted,filter='data')
(extracted/'Cargo.toml').write_text('[workspace]\nresolver="3"\nmembers=["aq-admission-0.1.8","aq-lifecycle-protocol-0.1.0","aq-session-protocol-0.1.0"]\n[patch.crates-io]\naq-lifecycle-protocol={path="aq-lifecycle-protocol-0.1.0"}\naq-session-protocol={path="aq-session-protocol-0.1.0"}\n')
env['CARGO_TARGET_DIR']='/tmp/granted-closure-extracted-target'
for args,name in [(['cargo','test','-p','aq-admission','--all-targets','--offline'],'tests'),(['cargo','clippy','-p','aq-admission','--all-targets','--all-features','--offline','--','-D','warnings'],'clippy'),(['cargo','fmt','-p','aq-admission','--','--check'],'fmt')]:run(args,extracted,name)
result={'source_revision':revision,'archive':str(archive),'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'consumer_root':str(extracted),'dependencies':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p,_ in artifacts[1:]},'old017_archive':{'path':'/home/jkern/dev/aq-worker/vendor/aq-admission-0.1.7.crate','sha256':'46283490c98c5b082aa8dbfd4ccde230db3d97828fcbd71ff5b0526837bae545'},'scope':'Pure DTO/schema/digest/correlation only. No opaque worker proof producer, SQL owner, current authorization, release, runtime adoption or provider execution.'}
pathlib.Path('/tmp/granted-closure-portable-report.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
