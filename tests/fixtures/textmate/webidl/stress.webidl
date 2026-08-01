// Web IDL fixture covering interfaces, dictionaries, callbacks, and namespaces.
[Exposed=(Window,Worker), SecureContext]
interface Catalog {
  constructor(optional CatalogOptions options = {});
  readonly attribute unsigned long size;
  iterable<DOMString, CatalogEntry>;
  getter CatalogEntry? get(DOMString id);
  Promise<CatalogEntry> create(EntryInit init);
  undefined remove(DOMString id);
};

partial interface Catalog {
  attribute EventHandler onentrychange;
};

interface mixin Timestamped {
  readonly attribute DOMHighResTimeStamp createdAt;
  readonly attribute DOMHighResTimeStamp updatedAt;
};

CatalogEntry includes Timestamped;

[Exposed=(Window,Worker)]
interface CatalogEntry : EventTarget {
  readonly attribute DOMString id;
  attribute DOMString name;
  attribute EntryState state;
  readonly attribute FrozenArray<DOMString> tags;
  Promise<boolean> save(optional SaveOptions options = {});
  undefined archive();
};

enum EntryState { "pending", "active", "archived" };

dictionary CatalogOptions {
  unsigned long maximumEntries = 1000;
  boolean persistent = true;
};

dictionary EntryInit {
  required DOMString name;
  EntryState state = "active";
  sequence<DOMString> tags = [];
};

dictionary SaveOptions {
  boolean validate = true;
  AbortSignal? signal = null;
};

callback EntryPredicate = boolean (CatalogEntry entry);
callback interface CatalogObserver {
  undefined entryChanged(CatalogEntry entry);
};

namespace CatalogUtilities {
  boolean isValidName(DOMString value);
  DOMString normalizeName(DOMString value);
};

exception CatalogException {
  DOMString message;
};

[Exposed=Window]
interface CatalogView0 {
  readonly attribute unsigned long index;
  readonly attribute DOMString label;
  Promise<sequence<CatalogEntry>> query(optional DOMString filter = "");
};

[Exposed=Window]
interface CatalogView1 {
  readonly attribute unsigned long index;
  readonly attribute DOMString label;
  Promise<sequence<CatalogEntry>> query(optional DOMString filter = "");
};

[Exposed=Window]
interface CatalogView2 {
  readonly attribute unsigned long index;
  readonly attribute DOMString label;
  Promise<sequence<CatalogEntry>> query(optional DOMString filter = "");
};

[Exposed=Window]
interface CatalogView3 {
  readonly attribute unsigned long index;
  readonly attribute DOMString label;
  Promise<sequence<CatalogEntry>> query(optional DOMString filter = "");
};

[Exposed=Window]
interface CatalogView4 {
  readonly attribute unsigned long index;
  readonly attribute DOMString label;
  Promise<sequence<CatalogEntry>> query(optional DOMString filter = "");
};

[Exposed=Window]
interface CatalogView5 {
  readonly attribute unsigned long index;
  readonly attribute DOMString label;
  Promise<sequence<CatalogEntry>> query(optional DOMString filter = "");
};

[Exposed=Window]
interface CatalogView6 {
  readonly attribute unsigned long index;
  readonly attribute DOMString label;
  Promise<sequence<CatalogEntry>> query(optional DOMString filter = "");
};

[Exposed=Window]
interface CatalogView7 {
  readonly attribute unsigned long index;
  readonly attribute DOMString label;
  Promise<sequence<CatalogEntry>> query(optional DOMString filter = "");
};

[Exposed=Window]
interface CatalogView8 {
  readonly attribute unsigned long index;
  readonly attribute DOMString label;
  Promise<sequence<CatalogEntry>> query(optional DOMString filter = "");
};

[Exposed=Window]
interface CatalogView9 {
  readonly attribute unsigned long index;
  readonly attribute DOMString label;
  Promise<sequence<CatalogEntry>> query(optional DOMString filter = "");
};

typedef (DOMString or unsigned long long) CatalogKey;
typedef record<DOMString, DOMString> Metadata;
typedef sequence<CatalogEntry> CatalogEntryList;

[Exposed=Worker]
interface WorkerCatalogView {
  readonly attribute DOMString workerId;
  Promise<CatalogEntryList> entries();
};
