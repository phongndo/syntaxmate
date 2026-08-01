[Exposed=Window]
interface CatalogEntry {
  readonly attribute DOMString id;
  attribute DOMString name;
  Promise<boolean> save(optional SaveOptions options = {});
};

dictionary SaveOptions {
  boolean validate = true;
  sequence<DOMString> tags = [];
};

callback EntryPredicate = boolean (CatalogEntry entry);
typedef unsigned long long EntryId;
