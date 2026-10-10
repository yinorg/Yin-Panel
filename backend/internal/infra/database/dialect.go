package database

import "gorm.io/gorm"

// quoteIdent quotes a SQL identifier with the active dialect's quoting, so raw
// SQL stays valid across SQLite (double quotes), MySQL (backticks) and
// PostgreSQL (double quotes). It is required for reserved words such as "user".
func quoteIdent(db *gorm.DB, name string) string {
	if db == nil || db.Statement == nil {
		return name
	}
	return db.Statement.Quote(name)
}
