use enumflags2::{bitflags, BitFlags};

/// A single option controlling how [`Client::bulk_insert_with_options`]
/// behaves, corresponding to a flag of .NET's
/// [`SqlBulkCopyOptions`](https://learn.microsoft.com/en-us/dotnet/api/system.data.sqlclient.sqlbulkcopyoptions).
///
/// [`Client::bulk_insert_with_options`]: crate::Client::bulk_insert_with_options
#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlBulkCopyOption {
    /// Preserve source identity values. When not set, the identity column
    /// (if any) is excluded from the generated column list and the server
    /// assigns identity values as usual. Note: unlike the other options,
    /// this isn't a TDS `INSERT BULK ... WITH (...)` keyword - the TDS
    /// bulk-load protocol has no such option. It instead controls whether
    /// this crate includes an identity column in the column list it sends,
    /// which is how SQL Server's own `BULK INSERT`/`SqlBulkCopy` preserve
    /// identity values too.
    KeepIdentity = 1 << 0,
    /// Check constraints while data is being inserted. By default,
    /// constraints are not checked.
    CheckConstraints = 1 << 1,
    /// Obtain a bulk update lock for the duration of the bulk copy
    /// operation. When not set, row locks are used.
    TableLock = 1 << 2,
    /// Preserve null values in the destination table regardless of the
    /// settings for default values. When not set, null values are replaced
    /// by default values where applicable.
    KeepNulls = 1 << 3,
    /// Cause the server to fire the insert triggers for the rows being
    /// inserted into the database. By default, triggers are not fired.
    FireTriggers = 1 << 4,
}

/// A set of [`SqlBulkCopyOption`] flags for [`Client::bulk_insert_with_options`].
/// Defaults to no options set, matching .NET's `SqlBulkCopyOptions.Default`.
///
/// [`Client::bulk_insert_with_options`]: crate::Client::bulk_insert_with_options
pub type SqlBulkCopyOptions = BitFlags<SqlBulkCopyOption>;

/// The sort order of a column, used to give [`Client::bulk_insert_with_options`]
/// an `ORDER` hint.
///
/// [`Client::bulk_insert_with_options`]: crate::Client::bulk_insert_with_options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortOrder {
    /// Ascending order (`ASC`).
    Ascending,
    /// Descending order (`DESC`).
    Descending,
}

/// A column name paired with its [`SortOrder`], hinting to the server that
/// the bulk-inserted rows already arrive sorted by this column - matching
/// an index on the destination table can measurably speed up the load. See
/// the `ORDER` clause of [`BULK INSERT`](https://learn.microsoft.com/en-us/sql/t-sql/statements/bulk-insert-transact-sql).
pub type ColumnOrderHint<'a> = (&'a str, SortOrder);
