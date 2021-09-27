use std::ops::{Bound, RangeBounds};
use std::{mem, ptr};

use crate::mdb::error::mdb_result;
use crate::mdb::ffi;
use crate::*;

/// A database that stores entries composed of slice of bytes.
///
/// # Example: Iterate over ranges of databases entries
///
/// In this example we store numbers in big endian this way those are ordered.
/// Thanks to their bytes representation, heed is able to iterate over them
/// from the lowest to the highest.
///
/// ```
/// # use std::fs;
/// # use std::path::Path;
/// # use heed::EnvOpenOptions;
/// use heed::Database;
/// use heed::types::*;
/// use heed::{zerocopy::I64, byteorder::BigEndian};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
/// # let env = EnvOpenOptions::new()
/// #     .map_size(10 * 1024 * 1024) // 10MB
/// #     .max_dbs(3000)
/// #     .open(Path::new("target").join("zerocopy.mdb"))?;
/// type BEI64 = I64<BigEndian>;
///
/// let db: Database = env.create_poly_database(Some("big-endian-iter"))?;
///
/// let mut wtxn = env.write_txn()?;
/// # db.clear(&mut wtxn)?;
/// db.put::<_, OwnedType<BEI64>, Unit>(&mut wtxn, &BEI64::new(0), &())?;
/// db.put::<_, OwnedType<BEI64>, Str>(&mut wtxn, &BEI64::new(35), "thirty five")?;
/// db.put::<_, OwnedType<BEI64>, Str>(&mut wtxn, &BEI64::new(42), "forty two")?;
/// db.put::<_, OwnedType<BEI64>, Unit>(&mut wtxn, &BEI64::new(68), &())?;
///
/// // you can iterate over database entries in order
/// let range = BEI64::new(35)..=BEI64::new(42);
/// let mut range = db.range::<_, OwnedType<BEI64>, Str, _>(&wtxn, &range)?;
/// assert_eq!(range.next().transpose()?, Some((BEI64::new(35), "thirty five")));
/// assert_eq!(range.next().transpose()?, Some((BEI64::new(42), "forty two")));
/// assert_eq!(range.next().transpose()?, None);
///
/// drop(range);
/// wtxn.commit()?;
/// # Ok(()) }
/// ```
///
/// # Example: Select ranges of entries
///
/// Heed also support ranges deletions.
/// Same configuration as above, numbers are ordered, therefore it is safe to specify
/// a range and be able to iterate over and/or delete it.
///
/// ```
/// # use std::fs;
/// # use std::path::Path;
/// # use heed::EnvOpenOptions;
/// use heed::Database;
/// use heed::types::*;
/// use heed::{zerocopy::I64, byteorder::BigEndian};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
/// # let env = EnvOpenOptions::new()
/// #     .map_size(10 * 1024 * 1024) // 10MB
/// #     .max_dbs(3000)
/// #     .open(Path::new("target").join("zerocopy.mdb"))?;
/// type BEI64 = I64<BigEndian>;
///
/// let db: Database = env.create_poly_database(Some("big-endian-iter"))?;
///
/// let mut wtxn = env.write_txn()?;
/// # db.clear(&mut wtxn)?;
/// db.put::<_, OwnedType<BEI64>, Unit>(&mut wtxn, &BEI64::new(0), &())?;
/// db.put::<_, OwnedType<BEI64>, Str>(&mut wtxn, &BEI64::new(35), "thirty five")?;
/// db.put::<_, OwnedType<BEI64>, Str>(&mut wtxn, &BEI64::new(42), "forty two")?;
/// db.put::<_, OwnedType<BEI64>, Unit>(&mut wtxn, &BEI64::new(68), &())?;
///
/// // even delete a range of keys
/// let range = BEI64::new(35)..=BEI64::new(42);
/// let deleted = db.delete_range::<_, OwnedType<BEI64>, _>(&mut wtxn, &range)?;
/// assert_eq!(deleted, 2);
///
/// let rets: Result<_, _> = db.iter::<_, OwnedType<BEI64>, Unit>(&wtxn)?.collect();
/// let rets: Vec<(BEI64, _)> = rets?;
///
/// let expected = vec![
///     (BEI64::new(0), ()),
///     (BEI64::new(68), ()),
/// ];
///
/// assert_eq!(deleted, 2);
/// assert_eq!(rets, expected);
///
/// wtxn.commit()?;
/// # Ok(()) }
/// ```
#[derive(Copy, Clone)]
pub struct Database {
    pub(crate) env_ident: usize,
    pub(crate) dbi: ffi::MDB_dbi,
}

impl Database {
    pub(crate) fn new(env_ident: usize, dbi: ffi::MDB_dbi) -> Database {
        Database { env_ident, dbi }
    }

    /// Retrieves the value associated with a key.
    ///
    /// If the key does not exist, then `None` is returned.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// let db = env.create_poly_database(Some("get-poly-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, Str, OwnedType<i32>>(&mut wtxn, "i-am-forty-two", &42)?;
    /// db.put::<_, Str, OwnedType<i32>>(&mut wtxn, "i-am-twenty-seven", &27)?;
    ///
    /// let ret = db.get::<_, Str, OwnedType<i32>>(&wtxn, "i-am-forty-two")?;
    /// assert_eq!(ret, Some(42));
    ///
    /// let ret = db.get::<_, Str, OwnedType<i32>>(&wtxn, "i-am-twenty-one")?;
    /// assert_eq!(ret, None);
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn get<'a, 'txn, T>(
        &self,
        txn: &'txn RoTxn<T>,
        key: &'a [u8],
    ) -> Result<Option<&'txn [u8]>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);

        let mut key_val = unsafe { crate::into_val(key) };
        let mut data_val = mem::MaybeUninit::uninit();

        let result = unsafe {
            mdb_result(ffi::mdb_get(
                txn.txn,
                self.dbi,
                &mut key_val,
                data_val.as_mut_ptr(),
            ))
        };

        match result {
            Ok(()) => {
                let data = unsafe { crate::from_val(data_val.assume_init()) };
                Ok(Some(data))
            }
            Err(e) if e.not_found() => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Retrieves the key/value pair lower than the given one in this database.
    ///
    /// If the database if empty or there is no key lower than the given one,
    /// then `None` is returned.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::U32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEU32 = U32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("get-lt-u32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(27), &())?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(42), &())?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(43), &())?;
    ///
    /// let ret = db.get_lower_than::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(4404))?;
    /// assert_eq!(ret, Some((BEU32::new(43), ())));
    ///
    /// let ret = db.get_lower_than::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(43))?;
    /// assert_eq!(ret, Some((BEU32::new(42), ())));
    ///
    /// let ret = db.get_lower_than::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(27))?;
    /// assert_eq!(ret, None);
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn get_lower_than<'a, 'txn, T>(
        &self,
        txn: &'txn RoTxn<T>,
        key: &'a [u8],
    ) -> Result<Option<(&'txn [u8], &'txn [u8])>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        let mut cursor = RoCursor::new(txn, self.dbi)?;
        cursor.move_on_key_greater_than_or_equal_to(key)?;
        cursor.move_on_prev()
    }

    /// Retrieves the key/value pair lower than or equal the given one in this database.
    ///
    /// If the database if empty or there is no key lower than or equal to the given one,
    /// then `None` is returned.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::U32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEU32 = U32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("get-lte-u32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(27), &())?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(42), &())?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(43), &())?;
    ///
    /// let ret = db.get_lower_than_or_equal_to::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(4404))?;
    /// assert_eq!(ret, Some((BEU32::new(43), ())));
    ///
    /// let ret = db.get_lower_than_or_equal_to::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(43))?;
    /// assert_eq!(ret, Some((BEU32::new(43), ())));
    ///
    /// let ret = db.get_lower_than_or_equal_to::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(26))?;
    /// assert_eq!(ret, None);
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn get_lower_than_or_equal_to<'a, 'txn, T>(
        &self,
        txn: &'txn RoTxn<T>,
        key: &'a [u8],
    ) -> Result<Option<(&'txn [u8], &'txn [u8])>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        let mut cursor = RoCursor::new(txn, self.dbi)?;
        match cursor.move_on_key_greater_than_or_equal_to(key) {
            Ok(Some((k, data))) if k == &key[..] => Ok(Some((k, data))),
            Ok(_) => cursor.move_on_prev(),
            Err(e) => Err(e),
        }
    }

    /// Retrieves the key/value pair greater than the given one in this database.
    ///
    /// If the database if empty or there is no key greater than the given one,
    /// then `None` is returned.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::U32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEU32 = U32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("get-lt-u32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(27), &())?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(42), &())?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(43), &())?;
    ///
    /// let ret = db.get_greater_than::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(0))?;
    /// assert_eq!(ret, Some((BEU32::new(27), ())));
    ///
    /// let ret = db.get_greater_than::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(42))?;
    /// assert_eq!(ret, Some((BEU32::new(43), ())));
    ///
    /// let ret = db.get_greater_than::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(43))?;
    /// assert_eq!(ret, None);
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn get_greater_than<'a, 'txn, T>(
        &self,
        txn: &'txn RoTxn<T>,
        key: &'a [u8],
    ) -> Result<Option<(&'txn [u8], &'txn [u8])>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        let mut cursor = RoCursor::new(txn, self.dbi)?;
        match cursor.move_on_key_greater_than_or_equal_to(key)? {
            Some((k, data)) if k > &key[..] => Ok(Some((k, data))),
            Some((_key, _data)) => cursor.move_on_next(),
            None => Ok(None),
        }
    }

    /// Retrieves the key/value pair greater than or equal the given one in this database.
    ///
    /// If the database if empty or there is no key greater than or equal to the given one,
    /// then `None` is returned.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::U32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEU32 = U32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("get-lt-u32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(27), &())?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(42), &())?;
    /// db.put::<_, OwnedType<BEU32>, Unit>(&mut wtxn, &BEU32::new(43), &())?;
    ///
    /// let ret = db.get_greater_than_or_equal_to::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(0))?;
    /// assert_eq!(ret, Some((BEU32::new(27), ())));
    ///
    /// let ret = db.get_greater_than_or_equal_to::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(42))?;
    /// assert_eq!(ret, Some((BEU32::new(42), ())));
    ///
    /// let ret = db.get_greater_than_or_equal_to::<_, OwnedType<BEU32>, Unit>(&wtxn, &BEU32::new(44))?;
    /// assert_eq!(ret, None);
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn get_greater_than_or_equal_to<'a, 'txn, T>(
        &self,
        txn: &'txn RoTxn<T>,
        key: &'a [u8],
    ) -> Result<Option<(&'txn [u8], &'txn [u8])>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        let mut cursor = RoCursor::new(txn, self.dbi)?;
        cursor.move_on_key_greater_than_or_equal_to(key)
    }

    /// Retrieves the first key/value pair of this database.
    ///
    /// If the database if empty, then `None` is returned.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("first-poly-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    ///
    /// let ret = db.first::<_, OwnedType<BEI32>, Str>(&wtxn)?;
    /// assert_eq!(ret, Some((BEI32::new(27), "i-am-twenty-seven")));
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn first<'txn, T>(&self, txn: &'txn RoTxn<T>) -> Result<Option<(&'txn [u8], &'txn [u8])>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        let mut cursor = RoCursor::new(txn, self.dbi)?;
        cursor.move_on_first()
    }

    /// Retrieves the last key/value pair of this database.
    ///
    /// If the database if empty, then `None` is returned.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("last-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    ///
    /// let ret = db.last::<_, OwnedType<BEI32>, Str>(&wtxn)?;
    /// assert_eq!(ret, Some((BEI32::new(42), "i-am-forty-two")));
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn last<'txn, T>(&self, txn: &'txn RoTxn<T>) -> Result<Option<(&'txn [u8], &'txn [u8])>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        let mut cursor = RoCursor::new(txn, self.dbi)?;
        cursor.move_on_last()
    }

    /// Returns the number of elements in this database.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// let ret = db.len(&wtxn)?;
    /// assert_eq!(ret, 4);
    ///
    /// db.delete::<_, OwnedType<BEI32>>(&mut wtxn, &BEI32::new(27))?;
    ///
    /// let ret = db.len(&wtxn)?;
    /// assert_eq!(ret, 3);
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn len<'txn, T>(&self, txn: &'txn RoTxn<T>) -> Result<usize> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);

        let mut cursor = RoCursor::new(txn, self.dbi)?;
        let mut count = 0;

        match cursor.move_on_first()? {
            Some(_) => count += 1,
            None => return Ok(0),
        }

        while let Some(_) = cursor.move_on_next()? {
            count += 1;
        }

        Ok(count)
    }

    /// Returns `true` if and only if this database is empty.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// let ret = db.is_empty(&wtxn)?;
    /// assert_eq!(ret, false);
    ///
    /// db.clear(&mut wtxn)?;
    ///
    /// let ret = db.is_empty(&wtxn)?;
    /// assert_eq!(ret, true);
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn is_empty<'txn, T>(&self, txn: &'txn RoTxn<T>) -> Result<bool> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);

        let mut cursor = RoCursor::new(txn, self.dbi)?;
        match cursor.move_on_first()? {
            Some(_) => Ok(false),
            None => Ok(true),
        }
    }

    /// Return a lexicographically ordered iterator of all key-value pairs in this database.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    ///
    /// let mut iter = db.iter::<_, OwnedType<BEI32>, Str>(&wtxn)?;
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(13), "i-am-thirteen")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(27), "i-am-twenty-seven")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(42), "i-am-forty-two")));
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn iter<'txn, T>(&self, txn: &'txn RoTxn<T>) -> Result<RoIter<'txn>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        RoCursor::new(txn, self.dbi).map(|cursor| RoIter::new(cursor))
    }

    /// Return a mutable lexicographically ordered iterator of all key-value pairs in this database.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    ///
    /// let mut iter = db.iter_mut::<_, OwnedType<BEI32>, Str>(&mut wtxn)?;
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(13), "i-am-thirteen")));
    /// let ret = unsafe { iter.del_current()? };
    /// assert!(ret);
    ///
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(27), "i-am-twenty-seven")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(42), "i-am-forty-two")));
    /// let ret = unsafe { iter.put_current(&BEI32::new(42), "i-am-the-new-forty-two")? };
    /// assert!(ret);
    ///
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    ///
    /// let ret = db.get::<_, OwnedType<BEI32>, Str>(&wtxn, &BEI32::new(13))?;
    /// assert_eq!(ret, None);
    ///
    /// let ret = db.get::<_, OwnedType<BEI32>, Str>(&wtxn, &BEI32::new(42))?;
    /// assert_eq!(ret, Some("i-am-the-new-forty-two"));
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn iter_mut<'txn, T>(&self, txn: &'txn mut RwTxn<T>) -> Result<RwIter<'txn>> {
        assert_eq!(self.env_ident, txn.txn.env.env_mut_ptr() as usize);
        RwCursor::new(txn, self.dbi).map(|cursor| RwIter::new(cursor))
    }

    /// Returns a reversed lexicographically ordered iterator of all key-value pairs in this database.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    ///
    /// let mut iter = db.rev_iter::<_, OwnedType<BEI32>, Str>(&wtxn)?;
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(42), "i-am-forty-two")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(27), "i-am-twenty-seven")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(13), "i-am-thirteen")));
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn rev_iter<'txn, T>(&self, txn: &'txn RoTxn<T>) -> Result<RoRevIter<'txn>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        RoCursor::new(txn, self.dbi).map(|cursor| RoRevIter::new(cursor))
    }

    /// Return a mutable reversed lexicographically ordered iterator of all key-value pairs
    /// in this database.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    ///
    /// let mut iter = db.rev_iter_mut::<_, OwnedType<BEI32>, Str>(&mut wtxn)?;
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(42), "i-am-forty-two")));
    /// let ret = unsafe { iter.del_current()? };
    /// assert!(ret);
    ///
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(27), "i-am-twenty-seven")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(13), "i-am-thirteen")));
    /// let ret = unsafe { iter.put_current(&BEI32::new(13), "i-am-the-new-thirteen")? };
    /// assert!(ret);
    ///
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    ///
    /// let ret = db.get::<_, OwnedType<BEI32>, Str>(&wtxn, &BEI32::new(42))?;
    /// assert_eq!(ret, None);
    ///
    /// let ret = db.get::<_, OwnedType<BEI32>, Str>(&wtxn, &BEI32::new(13))?;
    /// assert_eq!(ret, Some("i-am-the-new-thirteen"));
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn rev_iter_mut<'txn, T>(&self, txn: &'txn mut RwTxn<T>) -> Result<RwRevIter<'txn>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        RwCursor::new(txn, self.dbi).map(|cursor| RwRevIter::new(cursor))
    }

    /// Return a lexicographically ordered iterator of a range of key-value pairs in this database.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// let range = BEI32::new(27)..=BEI32::new(42);
    /// let mut iter = db.range::<_, OwnedType<BEI32>, Str, _>(&wtxn, &range)?;
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(27), "i-am-twenty-seven")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(42), "i-am-forty-two")));
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn range<'txn, T, R, A>(&self, txn: &'txn RoTxn<T>, range: R) -> Result<RoRange<'txn>>
    where
        R: RangeBounds<A>,
        A: AsRef<[u8]>,
    {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);

        let start_bound = match range.start_bound() {
            Bound::Included(bytes) => Bound::Included(bytes.as_ref().to_vec()),
            Bound::Excluded(bytes) => Bound::Excluded(bytes.as_ref().to_vec()),
            Bound::Unbounded => Bound::Unbounded,
        };

        let end_bound = match range.end_bound() {
            Bound::Included(bytes) => Bound::Included(bytes.as_ref().to_vec()),
            Bound::Excluded(bytes) => Bound::Excluded(bytes.as_ref().to_vec()),
            Bound::Unbounded => Bound::Unbounded,
        };

        RoCursor::new(txn, self.dbi).map(|cursor| RoRange::new(cursor, start_bound, end_bound))
    }

    /// Return a mutable lexicographically ordered iterator of a range of
    /// key-value pairs in this database.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// let range = BEI32::new(27)..=BEI32::new(42);
    /// let mut range = db.range_mut::<_, OwnedType<BEI32>, Str, _>(&mut wtxn, &range)?;
    /// assert_eq!(range.next().transpose()?, Some((BEI32::new(27), "i-am-twenty-seven")));
    /// let ret = unsafe { range.del_current()? };
    /// assert!(ret);
    /// assert_eq!(range.next().transpose()?, Some((BEI32::new(42), "i-am-forty-two")));
    /// let ret = unsafe { range.put_current(&BEI32::new(42), "i-am-the-new-forty-two")? };
    /// assert!(ret);
    ///
    /// assert_eq!(range.next().transpose()?, None);
    /// drop(range);
    ///
    ///
    /// let mut iter = db.iter::<_, OwnedType<BEI32>, Str>(&wtxn)?;
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(13), "i-am-thirteen")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(42), "i-am-the-new-forty-two")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(521), "i-am-five-hundred-and-twenty-one")));
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn range_mut<'txn, T, R, A>(
        &self,
        txn: &'txn mut RwTxn<T>,
        range: R,
    ) -> Result<RwRange<'txn>>
    where
        R: RangeBounds<A>,
        A: AsRef<[u8]>,
    {
        assert_eq!(self.env_ident, txn.txn.env.env_mut_ptr() as usize);

        let start_bound = match range.start_bound() {
            Bound::Included(bytes) => Bound::Included(bytes.as_ref().to_vec()),
            Bound::Excluded(bytes) => Bound::Excluded(bytes.as_ref().to_vec()),
            Bound::Unbounded => Bound::Unbounded,
        };

        let end_bound = match range.end_bound() {
            Bound::Included(bytes) => Bound::Included(bytes.as_ref().to_vec()),
            Bound::Excluded(bytes) => Bound::Excluded(bytes.as_ref().to_vec()),
            Bound::Unbounded => Bound::Unbounded,
        };

        RwCursor::new(txn, self.dbi).map(|cursor| RwRange::new(cursor, start_bound, end_bound))
    }

    /// Return a reversed lexicographically ordered iterator of a range of key-value
    /// pairs in this database.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// let range = BEI32::new(27)..=BEI32::new(43);
    /// let mut iter = db.rev_range::<_, OwnedType<BEI32>, Str, _>(&wtxn, &range)?;
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(42), "i-am-forty-two")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(27), "i-am-twenty-seven")));
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn rev_range<'txn, T, R, A>(
        &self,
        txn: &'txn RoTxn<T>,
        range: R,
    ) -> Result<RoRevRange<'txn>>
    where
        R: RangeBounds<A>,
        A: AsRef<[u8]>,
    {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);

        let start_bound = match range.start_bound() {
            Bound::Included(bytes) => Bound::Included(bytes.as_ref().to_vec()),
            Bound::Excluded(bytes) => Bound::Excluded(bytes.as_ref().to_vec()),
            Bound::Unbounded => Bound::Unbounded,
        };

        let end_bound = match range.end_bound() {
            Bound::Included(bytes) => Bound::Included(bytes.as_ref().to_vec()),
            Bound::Excluded(bytes) => Bound::Excluded(bytes.as_ref().to_vec()),
            Bound::Unbounded => Bound::Unbounded,
        };

        RoCursor::new(txn, self.dbi).map(|cursor| RoRevRange::new(cursor, start_bound, end_bound))
    }

    /// Return a mutable reversed lexicographically ordered iterator of a range of
    /// key-value pairs in this database.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// let range = BEI32::new(27)..=BEI32::new(42);
    /// let mut range = db.rev_range_mut::<_, OwnedType<BEI32>, Str, _>(&mut wtxn, &range)?;
    /// assert_eq!(range.next().transpose()?, Some((BEI32::new(42), "i-am-forty-two")));
    /// let ret = unsafe { range.del_current()? };
    /// assert!(ret);
    /// assert_eq!(range.next().transpose()?, Some((BEI32::new(27), "i-am-twenty-seven")));
    /// let ret = unsafe { range.put_current(&BEI32::new(27), "i-am-the-new-twenty-seven")? };
    /// assert!(ret);
    ///
    /// assert_eq!(range.next().transpose()?, None);
    /// drop(range);
    ///
    ///
    /// let mut iter = db.iter::<_, OwnedType<BEI32>, Str>(&wtxn)?;
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(13), "i-am-thirteen")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(27), "i-am-the-new-twenty-seven")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(521), "i-am-five-hundred-and-twenty-one")));
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn rev_range_mut<'txn, T, R, A>(
        &self,
        txn: &'txn mut RwTxn<T>,
        range: R,
    ) -> Result<RwRevRange<'txn>>
    where
        R: RangeBounds<A>,
        A: AsRef<[u8]>,
    {
        assert_eq!(self.env_ident, txn.txn.env.env_mut_ptr() as usize);

        let start_bound = match range.start_bound() {
            Bound::Included(bytes) => Bound::Included(bytes.as_ref().to_vec()),
            Bound::Excluded(bytes) => Bound::Excluded(bytes.as_ref().to_vec()),
            Bound::Unbounded => Bound::Unbounded,
        };

        let end_bound = match range.end_bound() {
            Bound::Included(bytes) => Bound::Included(bytes.as_ref().to_vec()),
            Bound::Excluded(bytes) => Bound::Excluded(bytes.as_ref().to_vec()),
            Bound::Unbounded => Bound::Unbounded,
        };

        RwCursor::new(txn, self.dbi).map(|cursor| RwRevRange::new(cursor, start_bound, end_bound))
    }

    /// Return a lexicographically ordered iterator of all key-value pairs
    /// in this database that starts with the given prefix.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-eight", &BEI32::new(28))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-seven", &BEI32::new(27))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-nine",  &BEI32::new(29))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-forty-one",    &BEI32::new(41))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-forty-two",    &BEI32::new(42))?;
    ///
    /// let mut iter = db.prefix_iter::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty")?;
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-eight", BEI32::new(28))));
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-nine", BEI32::new(29))));
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-seven", BEI32::new(27))));
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn prefix_iter<'a, 'txn, T>(
        &self,
        txn: &'txn RoTxn<T>,
        prefix: &'a [u8],
    ) -> Result<RoPrefix<'txn>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        RoCursor::new(txn, self.dbi).map(|cursor| RoPrefix::new(cursor, prefix.to_vec()))
    }

    /// Return a mutable lexicographically ordered iterator of all key-value pairs
    /// in this database that starts with the given prefix.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-eight", &BEI32::new(28))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-seven", &BEI32::new(27))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-nine",  &BEI32::new(29))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-forty-one",    &BEI32::new(41))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-forty-two",    &BEI32::new(42))?;
    ///
    /// let mut iter = db.prefix_iter_mut::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty")?;
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-eight", BEI32::new(28))));
    /// let ret = unsafe { iter.del_current()? };
    /// assert!(ret);
    ///
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-nine", BEI32::new(29))));
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-seven", BEI32::new(27))));
    /// let ret = unsafe { iter.put_current("i-am-twenty-seven", &BEI32::new(27000))? };
    /// assert!(ret);
    ///
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    ///
    /// let ret = db.get::<_, Str, OwnedType<BEI32>>(&wtxn, "i-am-twenty-eight")?;
    /// assert_eq!(ret, None);
    ///
    /// let ret = db.get::<_, Str, OwnedType<BEI32>>(&wtxn, "i-am-twenty-seven")?;
    /// assert_eq!(ret, Some(BEI32::new(27000)));
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn prefix_iter_mut<'a, 'txn, T>(
        &self,
        txn: &'txn mut RwTxn<T>,
        prefix: &'a [u8],
    ) -> Result<RwPrefix<'txn>> {
        assert_eq!(self.env_ident, txn.txn.env.env_mut_ptr() as usize);
        RwCursor::new(txn, self.dbi).map(|cursor| RwPrefix::new(cursor, prefix.to_vec()))
    }

    /// Return a reversed lexicographically ordered iterator of all key-value pairs
    /// in this database that starts with the given prefix.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-eight", &BEI32::new(28))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-seven", &BEI32::new(27))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-nine",  &BEI32::new(29))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-forty-one",    &BEI32::new(41))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-forty-two",    &BEI32::new(42))?;
    ///
    /// let mut iter = db.rev_prefix_iter::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty")?;
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-seven", BEI32::new(27))));
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-nine", BEI32::new(29))));
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-eight", BEI32::new(28))));
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn rev_prefix_iter<'a, 'txn, T>(
        &self,
        txn: &'txn RoTxn<T>,
        prefix: &'a [u8],
    ) -> Result<RoRevPrefix<'txn>> {
        assert_eq!(self.env_ident, txn.env.env_mut_ptr() as usize);
        RoCursor::new(txn, self.dbi).map(|cursor| RoRevPrefix::new(cursor, prefix.to_vec()))
    }

    /// Return a mutable lexicographically ordered iterator of all key-value pairs
    /// in this database that starts with the given prefix.
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-eight", &BEI32::new(28))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-seven", &BEI32::new(27))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty-nine",  &BEI32::new(29))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-forty-one",    &BEI32::new(41))?;
    /// db.put::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-forty-two",    &BEI32::new(42))?;
    ///
    /// let mut iter = db.rev_prefix_iter_mut::<_, Str, OwnedType<BEI32>>(&mut wtxn, "i-am-twenty")?;
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-seven", BEI32::new(27))));
    /// let ret = unsafe { iter.del_current()? };
    /// assert!(ret);
    ///
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-nine", BEI32::new(29))));
    /// assert_eq!(iter.next().transpose()?, Some(("i-am-twenty-eight", BEI32::new(28))));
    /// let ret = unsafe { iter.put_current("i-am-twenty-eight", &BEI32::new(28000))? };
    /// assert!(ret);
    ///
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    ///
    /// let ret = db.get::<_, Str, OwnedType<BEI32>>(&wtxn, "i-am-twenty-seven")?;
    /// assert_eq!(ret, None);
    ///
    /// let ret = db.get::<_, Str, OwnedType<BEI32>>(&wtxn, "i-am-twenty-eight")?;
    /// assert_eq!(ret, Some(BEI32::new(28000)));
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn rev_prefix_iter_mut<'a, 'txn, T>(
        &self,
        txn: &'txn mut RwTxn<T>,
        prefix: &'a [u8],
    ) -> Result<RwRevPrefix<'txn>> {
        assert_eq!(self.env_ident, txn.txn.env.env_mut_ptr() as usize);
        RwCursor::new(txn, self.dbi).map(|cursor| RwRevPrefix::new(cursor, prefix.to_vec()))
    }

    /// Insert a key-value pairs in this database.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// let ret = db.get::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27))?;
    /// assert_eq!(ret, Some("i-am-twenty-seven"));
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn put<'a, T>(&self, txn: &mut RwTxn<T>, key: &'a [u8], data: &'a [u8]) -> Result<()> {
        assert_eq!(self.env_ident, txn.txn.env.env_mut_ptr() as usize);

        let mut key_val = unsafe { crate::into_val(key) };
        let mut data_val = unsafe { crate::into_val(data) };
        let flags = 0;

        unsafe {
            mdb_result(ffi::mdb_put(
                txn.txn.txn,
                self.dbi,
                &mut key_val,
                &mut data_val,
                flags,
            ))?
        }

        Ok(())
    }

    /// Append the given key/data pair to the end of the database.
    ///
    /// This option allows fast bulk loading when keys are already known to be in the correct order.
    /// Loading unsorted keys will cause a MDB_KEYEXIST error.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("append-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// let ret = db.get::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27))?;
    /// assert_eq!(ret, Some("i-am-twenty-seven"));
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn append<'a, T>(&self, txn: &mut RwTxn<T>, key: &'a [u8], data: &'a [u8]) -> Result<()> {
        assert_eq!(self.env_ident, txn.txn.env.env_mut_ptr() as usize);

        let mut key_val = unsafe { crate::into_val(key) };
        let mut data_val = unsafe { crate::into_val(data) };
        let flags = ffi::MDB_APPEND;

        unsafe {
            mdb_result(ffi::mdb_put(
                txn.txn.txn,
                self.dbi,
                &mut key_val,
                &mut data_val,
                flags,
            ))?
        }

        Ok(())
    }

    /// Deletes a key-value pairs in this database.
    ///
    /// If the key does not exist, then `false` is returned.
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// let ret = db.delete::<_, OwnedType<BEI32>>(&mut wtxn, &BEI32::new(27))?;
    /// assert_eq!(ret, true);
    ///
    /// let ret = db.get::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27))?;
    /// assert_eq!(ret, None);
    ///
    /// let ret = db.delete::<_, OwnedType<BEI32>>(&mut wtxn, &BEI32::new(467))?;
    /// assert_eq!(ret, false);
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn delete<'a, T>(&self, txn: &mut RwTxn<T>, key: &'a [u8]) -> Result<bool> {
        assert_eq!(self.env_ident, txn.txn.env.env_mut_ptr() as usize);
        let mut key_val = unsafe { crate::into_val(key) };
        let result = unsafe {
            mdb_result(ffi::mdb_del(
                txn.txn.txn,
                self.dbi,
                &mut key_val,
                ptr::null_mut(),
            ))
        };

        match result {
            Ok(()) => Ok(true),
            Err(e) if e.not_found() => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Deletes a range of key-value pairs in this database.
    ///
    /// Perfer using [`clear`] instead of a call to this method with a full range ([`..`]).
    ///
    /// Comparisons are made by using the bytes representation of the key.
    ///
    /// [`clear`]: crate::Database::clear
    /// [`..`]: std::ops::RangeFull
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// let range = BEI32::new(27)..=BEI32::new(42);
    /// let ret = db.delete_range::<_, OwnedType<BEI32>, _>(&mut wtxn, &range)?;
    /// assert_eq!(ret, 2);
    ///
    /// let mut iter = db.iter::<_, OwnedType<BEI32>, Str>(&wtxn)?;
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(13), "i-am-thirteen")));
    /// assert_eq!(iter.next().transpose()?, Some((BEI32::new(521), "i-am-five-hundred-and-twenty-one")));
    /// assert_eq!(iter.next().transpose()?, None);
    ///
    /// drop(iter);
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn delete_range<'txn, T, R, A>(&self, txn: &'txn mut RwTxn<T>, range: R) -> Result<usize>
    where
        R: RangeBounds<A>,
        A: AsRef<[u8]>,
    {
        assert_eq!(self.env_ident, txn.txn.env.env_mut_ptr() as usize);

        let mut count = 0;
        let mut iter = self.range_mut::<T, _, _>(txn, range)?;

        while iter.next().is_some() {
            // safety: We do not keep any reference from the database while using `del_current`.
            //         The user can't keep any reference inside of the database as we ask for a
            //         mutable reference to the `txn`.
            unsafe { iter.del_current()? };
            count += 1;
        }

        Ok(count)
    }

    /// Deletes all key/value pairs in this database.
    ///
    /// Perfer using this method instead of a call to [`delete_range`] with a full range ([`..`]).
    ///
    /// [`delete_range`]: crate::Database::delete_range
    /// [`..`]: std::ops::RangeFull
    ///
    /// ```
    /// # use std::fs;
    /// # use std::path::Path;
    /// # use heed::EnvOpenOptions;
    /// use heed::Database;
    /// use heed::types::*;
    /// use heed::{zerocopy::I32, byteorder::BigEndian};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # fs::create_dir_all(Path::new("target").join("zerocopy.mdb"))?;
    /// # let env = EnvOpenOptions::new()
    /// #     .map_size(10 * 1024 * 1024) // 10MB
    /// #     .max_dbs(3000)
    /// #     .open(Path::new("target").join("zerocopy.mdb"))?;
    /// type BEI32 = I32<BigEndian>;
    ///
    /// let db = env.create_poly_database(Some("iter-i32"))?;
    ///
    /// let mut wtxn = env.write_txn()?;
    /// # db.clear(&mut wtxn)?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(42), "i-am-forty-two")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(27), "i-am-twenty-seven")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(13), "i-am-thirteen")?;
    /// db.put::<_, OwnedType<BEI32>, Str>(&mut wtxn, &BEI32::new(521), "i-am-five-hundred-and-twenty-one")?;
    ///
    /// db.clear(&mut wtxn)?;
    ///
    /// let ret = db.is_empty(&wtxn)?;
    /// assert!(ret);
    ///
    /// wtxn.commit()?;
    /// # Ok(()) }
    /// ```
    pub fn clear<T>(&self, txn: &mut RwTxn<T>) -> Result<()> {
        assert_eq!(self.env_ident, txn.txn.env.env_mut_ptr() as usize);
        unsafe { mdb_result(ffi::mdb_drop(txn.txn.txn, self.dbi, 0)).map_err(Into::into) }
    }
}
