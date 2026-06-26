//go:build !fake

package main

import (
	"flag"

	"github.com/f4ah6o/aistat/v2/internal/providers"
)

func registerFakeMode(_ *flag.FlagSet) func() []providers.Provider { return nil }
