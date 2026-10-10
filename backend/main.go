package main

import (
	"flag"
	"fmt"
	"log"
	"os"

	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/database"
	"github.com/yinorg/Yin-Panel/backend/pkg/app"
)

func main() {
	// `migrate-db` copies an existing SQLite deployment into MySQL/MariaDB or
	// PostgreSQL, then exits. It never starts the HTTP server.
	if len(os.Args) > 1 && os.Args[1] == "migrate-db" {
		runMigrateDB(os.Args[2:])
		return
	}

	// Parse command line arguments
	configPath := flag.String("c", "conf.yaml", "Path to configuration file")
	flag.Parse()

	// 打印 logo
	Logo()

	// Initialize the application with the specified config path
	if err := app.Init(*configPath); err != nil {
		log.Panicln("初始化错误:", err)
	}
}

// runMigrateDB copies every table from a SQLite configuration into a target
// MySQL/MariaDB or PostgreSQL configuration.
func runMigrateDB(args []string) {
	fs := flag.NewFlagSet("migrate-db", flag.ExitOnError)
	source := fs.String("source", "conf.yaml", "source SQLite configuration file")
	target := fs.String("target", "", "target MySQL/PostgreSQL configuration file")
	_ = fs.Parse(args)
	if *target == "" {
		fmt.Println("usage: yin-panel migrate-db -source conf.yaml -target conf.target.yaml")
		os.Exit(2)
	}
	fmt.Printf("Migrating from %s to %s ...\n", *source, *target)
	if err := database.MigrateDB(*source, *target); err != nil {
		log.Fatalln("migrate-db failed:", err)
	}
	fmt.Println("Migration complete.")
}

func Logo() {
	fmt.Println("__   __ _____ _   _   ____                  _")
	fmt.Println("\\ \\ / /| ____| \\ | | |  _ \\ __ _ _ __   ___| |")
	fmt.Println(" \\ V / |  _| |  \\| | | |_) / _` | '_ \\ / _ \\ |")
	fmt.Println("  | |  | |___| |\\  | |  __/ (_| | | | |  __/ |")
	fmt.Println("  |_|  |_____|_| \\_| |_|   \\__,_|_| |_|\\___|_|")
	if false {
		fmt.Println("     ____            ___                __")
		fmt.Println("    / __/_ _____    / _ \\___ ____  ___ / /")
		fmt.Println("   _\\ \\/ // / _ \\  / ___/ _ `/ _ \\/ -_) / ")
		fmt.Println("  /___/\\_,_/_//_/ /_/   \\_,_/_//_/\\__/_/  ")
	}
	fmt.Println("")

	fmt.Println("Version:", global.VERSION)
	fmt.Println("Welcome to the Yin-Panel.")
	fmt.Println("Project address:", "https://github.com/yinorg/Yin-Panel")
}
