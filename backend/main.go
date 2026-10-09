package main

import (
	"flag"
	"fmt"
	"log"

	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/pkg/app"
)

func main() {
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
