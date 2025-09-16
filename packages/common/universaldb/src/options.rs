#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum DatabaseOption {
	// /// Max location cache entries
	// ///
	// /// Set the size of the client location cache. Raising this value can boost performance in very large databases where clients access data in a near-random pattern. Defaults to 100000.
	// LocationCacheSize(i32),
	// /// Max outstanding watches
	// ///
	// /// Set the maximum number of watches allowed to be outstanding on a database connection. Increasing this number could result in increased resource usage. Reducing this number will not cancel any outstanding watches. Defaults to 10000 and cannot be larger than 1000000.
	// MaxWatches(i32),
	// /// Hexadecimal ID
	// ///
	// /// Specify the machine ID that was passed to fdbserver processes running on the same machine as this client, for better location-aware load balancing.
	// MachineId(String),
	// /// Hexadecimal ID
	// ///
	// /// Specify the datacenter ID that was passed to fdbserver processes running in the same datacenter as this client, for better location-aware load balancing.
	// DatacenterId(String),
	// /// Snapshot read operations will see the results of writes done in the same transaction. This is the default behavior.
	// SnapshotRywEnable,
	// /// Snapshot read operations will not see the results of writes done in the same transaction. This was the default behavior prior to API version 300.
	// SnapshotRywDisable,
	// /// Maximum length of escaped key and value fields.
	// ///
	// /// Sets the maximum escaped length of key and value fields to be logged to the trace file via the LOG_TRANSACTION option. This sets the ``transaction_logging_max_field_length`` option of each transaction created by this database. See the transaction option description for more information.
	// TransactionLoggingMaxFieldLength(i32),
	// /// value in milliseconds of timeout
	// ///
	// /// Set a timeout in milliseconds which, when elapsed, will cause each transaction automatically to be cancelled. This sets the ``timeout`` option of each transaction created by this database. See the transaction option description for more information. Using this option requires that the API version is 610 or higher.
	// TransactionTimeout(i32),
	/// number of times to retry
	///
	/// Set a maximum number of retries after which additional calls to ``onError`` will throw the most recently seen error code. This sets the ``retry_limit`` option of each transaction created by this database. See the transaction option description for more information.
	TransactionRetryLimit(i32),
	// /// value in milliseconds of maximum delay
	// ///
	// /// Set the maximum amount of backoff delay incurred in the call to ``onError`` if the error is retryable. This sets the ``max_retry_delay`` option of each transaction created by this database. See the transaction option description for more information.
	// TransactionMaxRetryDelay(i32),
	// /// value in bytes
	// ///
	// /// Set the maximum transaction size in bytes. This sets the ``size_limit`` option on each transaction created by this database. See the transaction option description for more information.
	// TransactionSizeLimit(i32),
	// /// The read version will be committed, and usually will be the latest committed, but might not be the latest committed in the event of a simultaneous fault and misbehaving clock.
	// TransactionCausalReadRisky,
	// /// Deprecated. Addresses returned by get_addresses_for_key include the port when enabled. As of api version 630, this option is enabled by default and setting this has no effect.
	// TransactionIncludePortInAddress,
	// /// Set a random idempotency id for all transactions. See the transaction option description for more information. This feature is in development and not ready for general use.
	// TransactionAutomaticIdempotency,
	// /// Allows ``get`` operations to read from sections of keyspace that have become unreadable because of versionstamp operations. This sets the ``bypass_unreadable`` option of each transaction created by this database. See the transaction option description for more information.
	// TransactionBypassUnreadable,
	// /// By default, operations that are performed on a transaction while it is being committed will not only fail themselves, but they will attempt to fail other in-flight operations (such as the commit) as well. This behavior is intended to help developers discover situations where operations could be unintentionally executed after the transaction has been reset. Setting this option removes that protection, causing only the offending operation to fail.
	// TransactionUsedDuringCommitProtectionDisable,
	// /// Enables conflicting key reporting on all transactions, allowing them to retrieve the keys that are conflicting with other transactions.
	// TransactionReportConflictingKeys,
	// /// Use configuration database.
	// UseConfigDatabase,
	// /// integer between 0 and 100 expressing the probability a client will verify it can't read stale data
	// ///
	// /// Enables verification of causal read risky by checking whether clients are able to read stale data when they detect a recovery, and logging an error if so.
	// TestCausalReadRisky(i32),
}
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum TransactionOption {
	/// The transaction, if not self-conflicting, may be committed a second time after commit succeeds, in the event of a fault
	CausalWriteRisky,
	/// The read version will be committed, and usually will be the latest committed, but might not be the latest committed in the event of a simultaneous fault and misbehaving clock.
	CausalReadRisky,
	CausalReadDisable,
	/// Addresses returned by get_addresses_for_key include the port when enabled. As of api version 630, this option is enabled by default and setting this has no effect.
	IncludePortInAddress,
	/// The next write performed on this transaction will not generate a write conflict range. As a result, other transactions which read the key(s) being modified by the next write will not conflict with this transaction. Care needs to be taken when using this option on a transaction that is shared between multiple threads. When setting this option, write conflict ranges will be disabled on the next write operation, regardless of what thread it is on.
	NextWriteNoWriteConflictRange,
	/// Reads performed by a transaction will not see any prior mutations that occured in that transaction, instead seeing the value which was in the database at the transaction's read version. This option may provide a small performance benefit for the client, but also disables a number of client-side optimizations which are beneficial for transactions which tend to read and write the same keys within a single transaction. It is an error to set this option after performing any reads or writes on the transaction.
	ReadYourWritesDisable,
	/// Deprecated
	ReadAheadDisable,
	/// Storage server should cache disk blocks needed for subsequent read requests in this transaction.  This is the default behavior.
	ReadServerSideCacheEnable,
	/// Storage server should not cache disk blocks needed for subsequent read requests in this transaction.  This can be used to avoid cache pollution for reads not expected to be repeated.
	ReadServerSideCacheDisable,
	/// Use normal read priority for subsequent read requests in this transaction.  This is the default read priority.
	ReadPriorityNormal,
	/// Use low read priority for subsequent read requests in this transaction.
	ReadPriorityLow,
	/// Use high read priority for subsequent read requests in this transaction.
	ReadPriorityHigh,
	DurabilityDatacenter,
	DurabilityRisky,
	/// Deprecated
	DurabilityDevNullIsWebScale,
	/// Specifies that this transaction should be treated as highest priority and that lower priority transactions should block behind this one. Use is discouraged outside of low-level tools
	PrioritySystemImmediate,
	/// Specifies that this transaction should be treated as low priority and that default priority transactions will be processed first. Batch priority transactions will also be throttled at load levels smaller than for other types of transactions and may be fully cut off in the event of machine failures. Useful for doing batch work simultaneously with latency-sensitive work
	PriorityBatch,
	/// This is a write-only transaction which sets the initial configuration. This option is designed for use by database system tools only.
	InitializeNewDatabase,
	/// Allows this transaction to read and modify system keys (those that start with the byte 0xFF). Implies raw_access.
	AccessSystemKeys,
	/// Allows this transaction to read system keys (those that start with the byte 0xFF). Implies raw_access.
	ReadSystemKeys,
	/// Allows this transaction to access the raw key-space when tenant mode is on.
	RawAccess,
	/// Allows this transaction to bypass storage quota enforcement. Should only be used for transactions that directly or indirectly decrease the size of the tenant group's data.
	BypassStorageQuota,
	/// Optional transaction name
	///
	DebugRetryLogging(String),
	/// String identifier to be used in the logs when tracing this transaction. The identifier must not exceed 100 characters.
	///
	/// Deprecated
	TransactionLoggingEnable(String),
	/// String identifier to be used when tracing or profiling this transaction. The identifier must not exceed 100 characters.
	///
	/// Sets a client provided identifier for the transaction that will be used in scenarios like tracing or profiling. Client trace logging or transaction profiling must be separately enabled.
	DebugTransactionIdentifier(String),
	/// Enables tracing for this transaction and logs results to the client trace logs. The DEBUG_TRANSACTION_IDENTIFIER option must be set before using this option, and client trace logging must be enabled to get log output.
	LogTransaction,
	/// Maximum length of escaped key and value fields.
	///
	/// Sets the maximum escaped length of key and value fields to be logged to the trace file via the LOG_TRANSACTION option, after which the field will be truncated. A negative value disables truncation.
	TransactionLoggingMaxFieldLength(i32),
	/// Sets an identifier for server tracing of this transaction. When committed, this identifier triggers logging when each part of the transaction authority encounters it, which is helpful in diagnosing slowness in misbehaving clusters. The identifier is randomly generated. When there is also a debug_transaction_identifier, both IDs are logged together.
	ServerRequestTracing,
	/// value in milliseconds of timeout
	///
	/// Set a timeout in milliseconds which, when elapsed, will cause the transaction automatically to be cancelled. Valid parameter values are ``[0, INT_MAX]``. If set to 0, will disable all timeouts. All pending and any future uses of the transaction will throw an exception. The transaction can be used again after it is reset. Prior to API version 610, like all other transaction options, the timeout must be reset after a call to ``onError``. If the API version is 610 or greater, the timeout is not reset after an ``onError`` call. This allows the user to specify a longer timeout on specific transactions than the default timeout specified through the ``transaction_timeout`` database option without the shorter database timeout cancelling transactions that encounter a retryable error. Note that at all API versions, it is safe and legal to set the timeout each time the transaction begins, so most code written assuming the older behavior can be upgraded to the newer behavior without requiring any modification, and the caller is not required to implement special logic in retry loops to only conditionally set this option.
	Timeout(i32),
	/// number of times to retry
	///
	/// Set a maximum number of retries after which additional calls to ``onError`` will throw the most recently seen error code. Valid parameter values are ``[-1, INT_MAX]``. If set to -1, will disable the retry limit. Prior to API version 610, like all other transaction options, the retry limit must be reset after a call to ``onError``. If the API version is 610 or greater, the retry limit is not reset after an ``onError`` call. Note that at all API versions, it is safe and legal to set the retry limit each time the transaction begins, so most code written assuming the older behavior can be upgraded to the newer behavior without requiring any modification, and the caller is not required to implement special logic in retry loops to only conditionally set this option.
	RetryLimit(i32),
	/// value in milliseconds of maximum delay
	///
	/// Set the maximum amount of backoff delay incurred in the call to ``onError`` if the error is retryable. Defaults to 1000 ms. Valid parameter values are ``[0, INT_MAX]``. If the maximum retry delay is less than the current retry delay of the transaction, then the current retry delay will be clamped to the maximum retry delay. Prior to API version 610, like all other transaction options, the maximum retry delay must be reset after a call to ``onError``. If the API version is 610 or greater, the retry limit is not reset after an ``onError`` call. Note that at all API versions, it is safe and legal to set the maximum retry delay each time the transaction begins, so most code written assuming the older behavior can be upgraded to the newer behavior without requiring any modification, and the caller is not required to implement special logic in retry loops to only conditionally set this option.
	MaxRetryDelay(i32),
	/// value in bytes
	///
	/// Set the transaction size limit in bytes. The size is calculated by combining the sizes of all keys and values written or mutated, all key ranges cleared, and all read and write conflict ranges. (In other words, it includes the total size of all data included in the request to the cluster to commit the transaction.) Large transactions can cause performance problems on FoundationDB clusters, so setting this limit to a smaller value than the default can help prevent the client from accidentally degrading the cluster's performance. This value must be at least 32 and cannot be set to higher than 10,000,000, the default transaction size limit.
	SizeLimit(i32),
	/// Automatically assign a random 16 byte idempotency id for this transaction. Prevents commits from failing with ``commit_unknown_result``. WARNING: If you are also using the multiversion client or transaction timeouts, if either cluster_version_changed or transaction_timed_out was thrown during a commit, then that commit may have already succeeded or may succeed in the future. This feature is in development and not ready for general use.
	AutomaticIdempotency,
	/// Snapshot read operations will see the results of writes done in the same transaction. This is the default behavior.
	SnapshotRywEnable,
	/// Snapshot read operations will not see the results of writes done in the same transaction. This was the default behavior prior to API version 300.
	SnapshotRywDisable,
	/// The transaction can read and write to locked databases, and is responsible for checking that it took the lock.
	LockAware,
	/// By default, operations that are performed on a transaction while it is being committed will not only fail themselves, but they will attempt to fail other in-flight operations (such as the commit) as well. This behavior is intended to help developers discover situations where operations could be unintentionally executed after the transaction has been reset. Setting this option removes that protection, causing only the offending operation to fail.
	UsedDuringCommitProtectionDisable,
	/// The transaction can read from locked databases.
	ReadLockAware,
	/// This option should only be used by tools which change the database configuration.
	UseProvisionalProxies,
	/// The transaction can retrieve keys that are conflicting with other transactions.
	ReportConflictingKeys,
	/// By default, the special key space will only allow users to read from exactly one module (a subspace in the special key space). Use this option to allow reading from zero or more modules. Users who set this option should be prepared for new modules, which may have different behaviors than the modules they're currently reading. For example, a new module might block or return an error.
	SpecialKeySpaceRelaxed,
	/// By default, users are not allowed to write to special keys. Enable this option will implicitly enable all options required to achieve the configuration change.
	SpecialKeySpaceEnableWrites,
	/// String identifier used to associated this transaction with a throttling group. Must not exceed 16 characters.
	///
	/// Adds a tag to the transaction that can be used to apply manual targeted throttling. At most 5 tags can be set on a transaction.
	Tag(String),
	/// String identifier used to associated this transaction with a throttling group. Must not exceed 16 characters.
	///
	/// Adds a tag to the transaction that can be used to apply manual or automatic targeted throttling. At most 5 tags can be set on a transaction.
	AutoThrottleTag(String),
	/// A byte string of length 16 used to associate the span of this transaction with a parent
	///
	/// Adds a parent to the Span of this transaction. Used for transaction tracing. A span can be identified with any 16 bytes
	SpanParent(Vec<u8>),
	/// Asks storage servers for how many bytes a clear key range contains. Otherwise uses the location cache to roughly estimate this.
	ExpensiveClearCostEstimationEnable,
	/// Allows ``get`` operations to read from sections of keyspace that have become unreadable because of versionstamp operations. These reads will view versionstamp operations as if they were set operations that did not fill in the versionstamp.
	BypassUnreadable,
	/// Allows this transaction to use cached GRV from the database context. Defaults to off. Upon first usage, starts a background updater to periodically update the cache to avoid stale read versions. The disable_client_bypass option must also be set.
	UseGrvCache,
	/// A JSON Web Token authorized to access data belonging to one or more tenants, indicated by 'tenants' claim of the token's payload.
	///
	/// Attach given authorization token to the transaction such that subsequent tenant-aware requests are authorized
	AuthorizationToken(String),
}
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum StreamingMode {
	/// Client intends to consume the entire range and would like it all transferred as early as possible.
	WantAll,
	/// The default. The client doesn't know how much of the range it is likely to used and wants different performance concerns to be balanced. Only a small portion of data is transferred to the client initially (in order to minimize costs if the client doesn't read the entire range), and as the caller iterates over more items in the range larger batches will be transferred in order to minimize latency. After enough iterations, the iterator mode will eventually reach the same byte limit as ``WANT_ALL``
	Iterator,
	/// Infrequently used. The client has passed a specific row limit and wants that many rows delivered in a single batch. Because of iterator operation in client drivers make request batches transparent to the user, consider ``WANT_ALL`` StreamingMode instead. A row limit must be specified if this mode is used.
	Exact,
	/// Infrequently used. Transfer data in batches small enough to not be much more expensive than reading individual rows, to minimize cost if iteration stops early.
	Small,
	/// Infrequently used. Transfer data in batches sized in between small and large.
	Medium,
	/// Infrequently used. Transfer data in batches large enough to be, in a high-concurrency environment, nearly as efficient as possible. If the client stops iteration early, some disk and network bandwidth may be wasted. The batch size may still be too small to allow a single client to get high throughput from the database, so if that is what you need consider the SERIAL StreamingMode.
	Large,
	/// Transfer data in batches large enough that an individual client can get reasonable read bandwidth from the database. If the client stops iteration early, considerable disk and network bandwidth may be wasted.
	Serial,
}
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum MutationType {
	/// addend
	///
	/// Performs an addition of little-endian integers. If the existing value in the database is not present or shorter than ``param``, it is first extended to the length of ``param`` with zero bytes.  If ``param`` is shorter than the existing value in the database, the existing value is truncated to match the length of ``param``. The integers to be added must be stored in a little-endian representation.  They can be signed in two's complement representation or unsigned. You can add to an integer at a known offset in the value by prepending the appropriate number of zero bytes to ``param`` and padding with zero bytes to match the length of the value. However, this offset technique requires that you know the addition will not cause the integer field within the value to overflow.
	Add,
	/// value with which to perform bitwise and
	///
	/// Deprecated
	And,
	/// value with which to perform bitwise and
	///
	/// Performs a bitwise ``and`` operation.  If the existing value in the database is not present, then ``param`` is stored in the database. If the existing value in the database is shorter than ``param``, it is first extended to the length of ``param`` with zero bytes.  If ``param`` is shorter than the existing value in the database, the existing value is truncated to match the length of ``param``.
	BitAnd,
	/// value with which to perform bitwise or
	///
	/// Deprecated
	Or,
	/// value with which to perform bitwise or
	///
	/// Performs a bitwise ``or`` operation.  If the existing value in the database is not present or shorter than ``param``, it is first extended to the length of ``param`` with zero bytes.  If ``param`` is shorter than the existing value in the database, the existing value is truncated to match the length of ``param``.
	BitOr,
	/// value with which to perform bitwise xor
	///
	/// Deprecated
	Xor,
	/// value with which to perform bitwise xor
	///
	/// Performs a bitwise ``xor`` operation.  If the existing value in the database is not present or shorter than ``param``, it is first extended to the length of ``param`` with zero bytes.  If ``param`` is shorter than the existing value in the database, the existing value is truncated to match the length of ``param``.
	BitXor,
	/// value to append to the database value
	///
	/// Appends ``param`` to the end of the existing value already in the database at the given key (or creates the key and sets the value to ``param`` if the key is empty). This will only append the value if the final concatenated value size is less than or equal to the maximum value size (i.e., if it fits). WARNING: No error is surfaced back to the user if the final value is too large because the mutation will not be applied until after the transaction has been committed. Therefore, it is only safe to use this mutation type if one can guarantee that one will keep the total value size under the maximum size.
	AppendIfFits,
	/// value to check against database value
	///
	/// Performs a little-endian comparison of byte strings. If the existing value in the database is not present or shorter than ``param``, it is first extended to the length of ``param`` with zero bytes.  If ``param`` is shorter than the existing value in the database, the existing value is truncated to match the length of ``param``. The larger of the two values is then stored in the database.
	Max,
	/// value to check against database value
	///
	/// Performs a little-endian comparison of byte strings. If the existing value in the database is not present, then ``param`` is stored in the database. If the existing value in the database is shorter than ``param``, it is first extended to the length of ``param`` with zero bytes.  If ``param`` is shorter than the existing value in the database, the existing value is truncated to match the length of ``param``. The smaller of the two values is then stored in the database.
	Min,
	/// value to which to set the transformed key
	///
	/// Transforms ``key`` using a versionstamp for the transaction. Sets the transformed key in the database to ``param``. The key is transformed by removing the final four bytes from the key and reading those as a little-Endian 32-bit integer to get a position ``pos``. The 10 bytes of the key from ``pos`` to ``pos + 10`` are replaced with the versionstamp of the transaction used. The first byte of the key is position 0. A versionstamp is a 10 byte, unique, monotonically (but not sequentially) increasing value for each committed transaction. The first 8 bytes are the committed version of the database (serialized in big-Endian order). The last 2 bytes are monotonic in the serialization order for transactions. WARNING: At this time, versionstamps are compatible with the Tuple layer only in the Java, Python, and Go bindings. Also, note that prior to API version 520, the offset was computed from only the final two bytes rather than the final four bytes.
	SetVersionstampedKey,
	/// value to versionstamp and set
	///
	/// Transforms ``param`` using a versionstamp for the transaction. Sets the ``key`` given to the transformed ``param``. The parameter is transformed by removing the final four bytes from ``param`` and reading those as a little-Endian 32-bit integer to get a position ``pos``. The 10 bytes of the parameter from ``pos`` to ``pos + 10`` are replaced with the versionstamp of the transaction used. The first byte of the parameter is position 0. A versionstamp is a 10 byte, unique, monotonically (but not sequentially) increasing value for each committed transaction. The first 8 bytes are the committed version of the database (serialized in big-Endian order). The last 2 bytes are monotonic in the serialization order for transactions. WARNING: At this time, versionstamps are compatible with the Tuple layer only in the Java, Python, and Go bindings. Also, note that prior to API version 520, the versionstamp was always placed at the beginning of the parameter rather than computing an offset.
	SetVersionstampedValue,
	/// value to check against database value
	///
	/// Performs lexicographic comparison of byte strings. If the existing value in the database is not present, then ``param`` is stored. Otherwise the smaller of the two values is then stored in the database.
	ByteMin,
	/// value to check against database value
	///
	/// Performs lexicographic comparison of byte strings. If the existing value in the database is not present, then ``param`` is stored. Otherwise the larger of the two values is then stored in the database.
	ByteMax,
	/// Value to compare with
	///
	/// Performs an atomic ``compare and clear`` operation. If the existing value in the database is equal to the given value, then given key is cleared.
	CompareAndClear,
}
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum ConflictRangeType {
	/// Used to add a read conflict range
	Read,
	/// Used to add a write conflict range
	Write,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorPredicate {
	/// Returns ``true`` if the error indicates the operations in the transactions should be retried because of transient error.
	Retryable,
	/// Returns ``true`` if the error indicates the transaction may have succeeded, though not in a way the system can verify.
	MaybeCommitted,
	/// Returns ``true`` if the error indicates the transaction has not committed, though in a way that can be retried.
	RetryableNotCommitted,
}
