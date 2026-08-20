use crate::Storage;
use anyhow::Result;

pub fn touch(name: String) -> Result<()> {
    let mut storage = Storage::load()?;
    storage.update_access(&name)?;
    Ok(())
}
