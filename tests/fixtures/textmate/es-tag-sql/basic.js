const accountId = 42;
const state = "active";

const account = sql`
  SELECT id, display_name, 'café 東京 🚀 𝌆' AS sample
  FROM accounts
  WHERE id = ${accountId}
    AND state = ${state}
  ORDER BY display_name ASC;
`;

const audit = db.sql`
  INSERT INTO audit_log (account_id, message)
  VALUES (${accountId}, 'opened account');
`;

const hostResult = { account, audit, complete: true };
export default hostResult;
