package main

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
)

var binPath string

func TestMain(m *testing.M) {
	dir, err := os.MkdirTemp("", "usage-check-test-")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(dir)
	binPath = filepath.Join(dir, "usage-check")
	out, err := exec.Command("go", "build", "-o", binPath, ".").CombinedOutput()
	if err != nil {
		os.Stderr.Write(out)
		panic("go build failed: " + err.Error())
	}
	os.Exit(m.Run())
}

type runResult struct {
	stdout string
	stderr string
	code   int
}

func runBin(args ...string) runResult {
	cmd := exec.Command(binPath, args...)
	var stdout, stderr strings.Builder
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	code := 0
	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			code = exitErr.ExitCode()
		} else {
			panic(err)
		}
	}
	return runResult{stdout.String(), stderr.String(), code}
}

func TestCLI_FakeJSON_All(t *testing.T) {
	r := runBin("--fake")
	if r.code != 0 {
		t.Fatalf("exit %d, stderr: %s", r.code, r.stderr)
	}
	var got map[string]any
	if err := json.Unmarshal([]byte(r.stdout), &got); err != nil {
		t.Fatalf("invalid JSON: %v\noutput: %s", err, r.stdout)
	}
	if _, ok := got["checked_at"]; !ok {
		t.Fatal("missing checked_at")
	}
	provs, _ := got["providers"].(map[string]any)
	for _, id := range []string{"claude", "codex", "copilot"} {
		if _, ok := provs[id]; !ok {
			t.Errorf("missing provider %s", id)
		}
	}
}

func TestCLI_FakeJSON_SingleProvider(t *testing.T) {
	r := runBin("--fake", "claude")
	if r.code != 0 {
		t.Fatalf("exit %d, stderr: %s", r.code, r.stderr)
	}
	var got map[string]any
	_ = json.Unmarshal([]byte(r.stdout), &got)
	provs, _ := got["providers"].(map[string]any)
	if _, ok := provs["claude"]; !ok {
		t.Error("missing claude")
	}
	if _, ok := provs["codex"]; ok {
		t.Error("codex should be absent when single provider requested")
	}
	if _, ok := got["checked_at"]; !ok {
		t.Error("checked_at must always be present")
	}
}

func TestCLI_FakeText_DesignSampleShape(t *testing.T) {
	r := runBin("--fake", "-h")
	if r.code != 0 {
		t.Fatalf("exit %d, stderr: %s", r.code, r.stderr)
	}
	// Provider order: Claude → Codex → Copilot.
	iC := strings.Index(r.stdout, "Claude usage")
	iCx := strings.Index(r.stdout, "Codex usage")
	iCp := strings.Index(r.stdout, "Copilot usage")
	if !(iC >= 0 && iC < iCx && iCx < iCp) {
		t.Fatalf("wrong section order:\n%s", r.stdout)
	}
	// Sanity-check one line shape with the design's format.
	if !regexp.MustCompile(`- 5-hour: \d+\.\d% \(resets in [^\)]+\)`).MatchString(r.stdout) {
		t.Fatalf("5-hour line missing or malformed:\n%s", r.stdout)
	}
}

func TestCLI_PositionalBeforeFlag(t *testing.T) {
	r := runBin("--fake", "claude", "-h")
	if r.code != 0 {
		t.Fatalf("exit %d, stderr: %s", r.code, r.stderr)
	}
	if !strings.Contains(r.stdout, "Claude usage") {
		t.Fatalf("missing Claude section: %s", r.stdout)
	}
	if strings.Contains(r.stdout, "Codex usage") {
		t.Fatalf("Codex should be absent: %s", r.stdout)
	}
}

func TestCLI_PositionalAfterFlag(t *testing.T) {
	r := runBin("--fake", "-h", "claude")
	if r.code != 0 || !strings.Contains(r.stdout, "Claude usage") {
		t.Fatalf("exit %d stdout %s", r.code, r.stdout)
	}
}

func TestCLI_HumanLongForm(t *testing.T) {
	a := runBin("--fake", "-h").stdout
	b := runBin("--fake", "--human").stdout
	if a != b {
		t.Fatalf("-h and --human should match:\n%s\nvs\n%s", a, b)
	}
}

func TestCLI_BogusPositional(t *testing.T) {
	r := runBin("--fake", "bogus")
	if r.code != 2 {
		t.Fatalf("expected exit 2, got %d", r.code)
	}
	if r.stdout != "" {
		t.Fatalf("stdout should be empty on error: %s", r.stdout)
	}
	if !strings.Contains(r.stderr, "unexpected argument: bogus") {
		t.Fatalf("missing actionable error: %s", r.stderr)
	}
	if !strings.Contains(r.stderr, "claude") || !strings.Contains(r.stderr, "codex") || !strings.Contains(r.stderr, "copilot") {
		t.Fatalf("error should name valid providers: %s", r.stderr)
	}
}

func TestCLI_UnknownFlag(t *testing.T) {
	r := runBin("--fake", "--unknown")
	if r.code != 2 {
		t.Fatalf("expected exit 2, got %d", r.code)
	}
	if r.stdout != "" {
		t.Fatalf("stdout should be empty: %s", r.stdout)
	}
	if !strings.Contains(r.stderr, "flag provided but not defined") {
		t.Fatalf("missing parse error: %s", r.stderr)
	}
}

func TestCLI_DroppedJSONFlag(t *testing.T) {
	r := runBin("--json")
	if r.code != 2 {
		t.Fatalf("expected exit 2, got %d", r.code)
	}
	if r.stdout != "" {
		t.Fatalf("stdout must be empty (no help-block leak from Usage): %s", r.stdout)
	}
	if !strings.Contains(r.stderr, "flag provided but not defined: -json") &&
		!strings.Contains(r.stderr, "flag provided but not defined: --json") {
		t.Fatalf("missing parse error: %s", r.stderr)
	}
}

func TestCLI_Help(t *testing.T) {
	r := runBin("--help")
	if r.code != 0 {
		t.Fatalf("expected exit 0, got %d", r.code)
	}
	if !strings.Contains(r.stdout, "usage-check") {
		t.Fatalf("help missing program name: %s", r.stdout)
	}
	if !strings.Contains(r.stdout, "-h, --human") {
		t.Fatalf("help missing -h, --human: %s", r.stdout)
	}
	if r.stderr != "" {
		t.Fatalf("stderr should be empty on --help: %s", r.stderr)
	}
}

func TestCLI_TwoPositionals(t *testing.T) {
	r := runBin("--fake", "claude", "codex")
	if r.code != 2 {
		t.Fatalf("expected exit 2 for two providers, got %d", r.code)
	}
	if !strings.Contains(r.stderr, "multiple providers") {
		t.Fatalf("missing multi-provider error: %s", r.stderr)
	}
}
