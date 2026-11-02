//! All protected bytes have immutable scope/request/slot-revision AAD.
use super::*;
use crate::protected_storage::Sealed;
use sha2::{Digest as _, Sha256};
use sqlx::{Postgres, Row, Transaction};
use zeroize::Zeroizing;
pub(super) const KINDS: [&str; 8] = [
    "original",
    "snapshot",
    "work",
    "registration",
    "cancellation",
    "outcome",
    "resume",
    "completion",
];
pub(super) fn bound(kind: &str) -> Result<usize> {
    Ok(match kind {
        "original" => 131072,
        "snapshot" | "work" => 262144,
        "registration" | "cancellation" | "completion" => 16384,
        "outcome" => 131072,
        "resume" => 8192,
        _ => return Err(Error::Corrupt),
    })
}
pub(super) fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn aad(wait: Uuid, request: &str, kind: &str, revision: u64) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(&(
        "aq.wait.internal.slot.1",
        wait,
        request,
        kind,
        revision,
    ))?)
}
impl Owner {
    pub(super) async fn empty_slots(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        wait: Uuid,
    ) -> Result<()> {
        for kind in KINDS {
            sqlx::query("INSERT INTO wait_owner_slots(wait_id,kind,revision,plaintext_bytes) VALUES($1,$2,0,0)")
            .bind(wait).bind(kind).execute(&mut **tx).await?;
        }
        Ok(())
    }
    pub(super) async fn put_slot(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        wait: Uuid,
        request: &str,
        kind: &str,
        revision: u64,
        bytes: &[u8],
    ) -> Result<()> {
        if bytes.is_empty() || bytes.len() > bound(kind)? {
            return Err(Error::Capacity);
        }
        let sealed = self
            .keys
            .seal(&aad(wait, request, kind, revision)?, bytes)
            .map_err(|_| Error::Protection)?;
        let changed = sqlx::query("UPDATE wait_owner_slots SET revision=$3,plaintext_bytes=$4,plaintext_digest=$5,sealed_version=$6,key_id=$7,nonce=$8,ciphertext=$9,ciphertext_digest=$10 WHERE wait_id=$1 AND kind=$2")
            .bind(wait).bind(kind).bind(i64::try_from(revision).map_err(|_|Error::Capacity)?).bind(bytes.len() as i64)
            .bind(digest(bytes)).bind(sealed.version as i32).bind(sealed.key_id).bind(sealed.nonce)
            .bind(&sealed.ciphertext).bind(digest(&sealed.ciphertext)).execute(&mut **tx).await?.rows_affected();
        if changed != 1 {
            return Err(Error::Corrupt);
        }
        Ok(())
    }
    pub(super) async fn get_slot(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        wait: Uuid,
        request: &str,
        kind: &str,
    ) -> Result<Option<Zeroizing<Vec<u8>>>> {
        let row=sqlx::query("SELECT revision,plaintext_bytes,plaintext_digest,sealed_version,key_id,nonce,ciphertext,ciphertext_digest FROM wait_owner_slots WHERE wait_id=$1 AND kind=$2")
            .bind(wait).bind(kind).fetch_optional(&mut **tx).await?.ok_or(Error::Corrupt)?;
        let size: i64 = row.get("plaintext_bytes");
        if size == 0 {
            return Ok(None);
        }
        if size < 0 || size as usize > bound(kind)? {
            return Err(Error::Corrupt);
        }
        let revision: i64 = row.get("revision");
        let sealed = Sealed {
            version: row
                .get::<Option<i32>, _>("sealed_version")
                .ok_or(Error::Corrupt)? as u32,
            key_id: row
                .get::<Option<String>, _>("key_id")
                .ok_or(Error::Corrupt)?,
            nonce: row
                .get::<Option<Vec<u8>>, _>("nonce")
                .ok_or(Error::Corrupt)?,
            ciphertext: row
                .get::<Option<Vec<u8>>, _>("ciphertext")
                .ok_or(Error::Corrupt)?,
        };
        if sealed.ciphertext.len() != size as usize + 16
            || row.get::<Option<String>, _>("ciphertext_digest").as_deref()
                != Some(digest(&sealed.ciphertext).as_str())
        {
            return Err(Error::Corrupt);
        }
        let bytes = self
            .keys
            .open(
                &aad(
                    wait,
                    request,
                    kind,
                    revision.try_into().map_err(|_| Error::Corrupt)?,
                )?,
                &sealed,
            )
            .map_err(|_| Error::Protection)?;
        if bytes.len() != size as usize
            || row.get::<Option<String>, _>("plaintext_digest").as_deref()
                != Some(digest(&bytes).as_str())
        {
            return Err(Error::Corrupt);
        }
        Ok(Some(bytes))
    }
}
