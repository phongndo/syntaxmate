const tenantId = 17;
const userId = 42;
const status = "active";
const limit = 25;
const offset = 50;
const client = { sql: String.raw };
const reporting = { sql: String.raw };

const userLookup = sql`
  SELECT
    u.id,
    u.display_name,
    u.email,
    u.created_at,
    COALESCE(p.locale, 'ja-JP') AS locale
  FROM app_user AS u
  LEFT JOIN user_profile AS p
    ON p.user_id = u.id
  WHERE u.tenant_id = ${tenantId}
    AND u.id = ${userId}
    AND u.status = ${status}
  ORDER BY u.created_at DESC
  LIMIT ${limit}
  OFFSET ${offset};
`;
const afterUserLookup = userLookup.length;

const insertAudit = inline-sql`
  INSERT INTO audit_event (
    tenant_id,
    actor_id,
    event_type,
    payload,
    created_at
  )
  VALUES (
    ${tenantId},
    ${userId},
    'profile.opened',
    '{"label":"café 東京 🚀 𝌆"}',
    CURRENT_TIMESTAMP
  )
  RETURNING id, created_at;
`;
const afterInsert = { text: insertAudit, inserted: true };

const monthlyReport = reporting.sql`
  WITH monthly_sales AS (
    SELECT
      date_trunc('month', paid_at) AS month,
      customer_id,
      SUM(total_cents) AS total_cents,
      COUNT(*) AS order_count
    FROM orders
    WHERE tenant_id = ${tenantId}
      AND paid_at >= ${new Date("2025-01-01")}
    GROUP BY date_trunc('month', paid_at), customer_id
  ), ranked AS (
    SELECT
      month,
      customer_id,
      total_cents,
      order_count,
      dense_rank() OVER (
        PARTITION BY month
        ORDER BY total_cents DESC
      ) AS sales_rank
    FROM monthly_sales
  )
  SELECT month, customer_id, total_cents, order_count, sales_rank
  FROM ranked
  WHERE sales_rank <= 10
  ORDER BY month DESC, sales_rank ASC;
`;
const afterMonthlyReport = monthlyReport.trim();

const updatePreferences = client.sql`
  UPDATE user_profile
  SET
    locale = ${"ja-JP"},
    timezone = ${"Asia/Tokyo"},
    preferences = jsonb_set(
      COALESCE(preferences, '{}'::jsonb),
      '{theme}',
      to_jsonb(${"dark"}::text),
      true
    ),
    updated_at = now()
  WHERE tenant_id = ${tenantId}
    AND user_id = ${userId}
  RETURNING user_id, locale, timezone;
`;
const afterUpdate = updatePreferences.includes("RETURNING");

const schemaSetup = /* sql */ `
  CREATE TABLE IF NOT EXISTS translation_entry (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tenant_id BIGINT NOT NULL,
    locale VARCHAR(16) NOT NULL,
    message_key TEXT NOT NULL,
    message_value TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT translation_entry_unique UNIQUE (tenant_id, locale, message_key)
  );

  CREATE INDEX IF NOT EXISTS translation_entry_lookup
    ON translation_entry (tenant_id, locale, message_key);

  COMMENT ON TABLE translation_entry
    IS 'Translations such as café, 東京, 🚀, and 𝌆';
`;
const afterSchema = schemaSetup.split(";").length;

const transaction = /* inline-sql */ `
  BEGIN;
  SAVEPOINT before_account_change;

  UPDATE account
  SET balance_cents = balance_cents - ${1250}
  WHERE tenant_id = ${tenantId}
    AND id = ${1001};

  UPDATE account
  SET balance_cents = balance_cents + ${1250}
  WHERE tenant_id = ${tenantId}
    AND id = ${1002};

  RELEASE SAVEPOINT before_account_change;
  COMMIT;
`;
const afterTransaction = Boolean(transaction);

const commentSelected =
  // sql
  `SELECT product_id, sku, title
   FROM catalog_product
   WHERE tenant_id = ${tenantId}
     AND title LIKE ${"%café%"}
   ORDER BY title COLLATE "C";
  `;
const afterCommentSelected = commentSelected.length;

const inlineCommentSelected =
  // inline-sql
  `DELETE FROM expired_session
   WHERE tenant_id = ${tenantId}
     AND expires_at < CURRENT_TIMESTAMP
   RETURNING session_id;
  `;
const afterInlineComment = inlineCommentSelected.trim();

const escapedText = sql`
  SELECT
    'single quote ''inside'' SQL text' AS quoted_text,
    E'backslash\\path' AS escaped_path,
    'café 東京 🚀 𝌆' AS unicode_text;
  -- SQL comments can mention punctuation: /* */ -- and ; safely.
  SELECT "MixedCaseIdentifier" FROM "ExampleTable";
`;
const afterEscapedText = escapedText.length > 0;

const analyticalQuery = client.sql`
  SELECT
    department,
    employee_id,
    salary,
    AVG(salary) OVER (PARTITION BY department) AS department_average,
    salary - AVG(salary) OVER (PARTITION BY department) AS difference,
    ROW_NUMBER() OVER (
      PARTITION BY department
      ORDER BY salary DESC, employee_id
    ) AS row_number
  FROM employee
  WHERE hired_at BETWEEN ${"2020-01-01"} AND ${"2025-12-31"}
    AND department IN ('Design', 'Engineering', 'Research')
  ORDER BY department, row_number;
`;
const afterAnalytical = analyticalQuery.slice(0, 16);

const recursiveQuery = sql`
  WITH RECURSIVE category_tree AS (
    SELECT id, parent_id, name, 0 AS depth
    FROM category
    WHERE parent_id IS NULL
    UNION ALL
    SELECT child.id, child.parent_id, child.name, tree.depth + 1
    FROM category AS child
    JOIN category_tree AS tree ON child.parent_id = tree.id
    WHERE tree.depth < ${8}
  )
  SELECT id, parent_id, name, depth
  FROM category_tree
  ORDER BY depth, name;
`;
const afterRecursive = recursiveQuery.endsWith("\n");

function summarizeSql() {
  return {
    afterUserLookup,
    afterInsert,
    afterMonthlyReport,
    afterUpdate,
    afterSchema,
    afterTransaction,
    afterCommentSelected,
    afterInlineComment,
    afterEscapedText,
    afterAnalytical,
    afterRecursive,
  };
}

export { userLookup, insertAudit, monthlyReport, updatePreferences, schemaSetup };
export default summarizeSql;
