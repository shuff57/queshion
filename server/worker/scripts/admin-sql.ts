// Prints the SQL for scripts/make-admin.sh and scripts/demote-admin.sh. The statements come from the
// auth package (the same function its tests run), so the eligibility rules are tested where used.
//   node --experimental-strip-types scripts/admin-sql.ts make EMAIL
//   node --experimental-strip-types scripts/admin-sql.ts demote EMAIL
import { demoteAdminStatements, makeAdminStatements } from "../../auth/src/bootstrap.ts";

const [cmd, email] = process.argv.slice(2);
let stmts: string[];
if (cmd === "make") stmts = makeAdminStatements({ email: email ?? "", adminLevel: "admin", fromLevels: ["pending", "teacher"] });
else if (cmd === "demote") stmts = demoteAdminStatements({ email: email ?? "", adminLevels: ["admin"], toLevel: "teacher" });
else {
  console.error("usage: admin-sql.ts make|demote EMAIL");
  process.exit(2);
}
process.stdout.write(stmts.map((s) => s.trim()).join(";\n") + ";\n");
