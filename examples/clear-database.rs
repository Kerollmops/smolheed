use std::error::Error;
use std::fs;
use std::path::Path;

use smolheed::EnvOpenOptions;

// In this test we are checking that we can clear database entries and
// write just after in the same transaction without loosing the writes.
fn main() -> Result<(), Box<dyn Error>> {
    let env_path = Path::new("target").join("clear-database.mdb");

    let _ = fs::remove_dir_all(&env_path);

    fs::create_dir_all(&env_path)?;
    let env = EnvOpenOptions::new()
        .map_size(10 * 1024 * 1024) // 10MB
        .max_dbs(3)
        .open(env_path)?;

    let db = env.create_database(Some("first"))?;
    let mut wtxn = env.write_txn()?;

    // We fill the db database with entries.
    db.put(&mut wtxn, b"I am here", b"to test things")?;
    db.put(&mut wtxn, b"I am here too", b"for the same purpose")?;

    wtxn.commit()?;

    let mut wtxn = env.write_txn()?;
    db.clear(&mut wtxn)?;
    db.put(&mut wtxn, b"And I come back", b"to test things")?;

    let mut iter = db.iter(&wtxn)?;
    assert_eq!(iter.next().transpose()?, Some((&b"And I come back"[..], &b"to test things"[..])));
    assert_eq!(iter.next().transpose()?, None);

    drop(iter);
    wtxn.commit()?;

    let rtxn = env.read_txn()?;
    let mut iter = db.iter(&rtxn)?;
    assert_eq!(iter.next().transpose()?, Some((&b"And I come back"[..], &b"to test things"[..])));
    assert_eq!(iter.next().transpose()?, None);

    Ok(())
}
