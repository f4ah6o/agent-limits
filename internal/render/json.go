package render

import (
	"encoding/json"
	"io"

	"github.com/f4ah6o/aistat/v2/internal/providers"
)

// JSON writes the report as indented JSON. encoding/json sorts map keys
// alphabetically, which yields claude/codex/copilot in the documented order.
func JSON(w io.Writer, r providers.Report) error {
	enc := json.NewEncoder(w)
	enc.SetIndent("", "  ")
	enc.SetEscapeHTML(false)
	return enc.Encode(r)
}
