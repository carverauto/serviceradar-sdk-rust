// Package runtime runs Rust SDK plugins the way the ServiceRadar agent does:
// wazero with WASI walltime, nanotime and nanosleep enabled and _start
// suppressed, calling the plugin's exported run_check directly.
package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"strings"
	"testing"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
)

type capture struct {
	config  []byte
	results [][]byte
	logs    []string
}

func (c *capture) instantiateEnv(ctx context.Context, rt wazero.Runtime) error {
	_, err := rt.NewHostModuleBuilder("env").
		NewFunctionBuilder().WithFunc(func(_ context.Context, m api.Module, ptr, size uint32) int32 {
		if len(c.config) == 0 {
			return 0
		}
		if uint32(len(c.config)) > size {
			return -1
		}
		m.Memory().Write(ptr, c.config)
		return int32(len(c.config))
	}).Export("get_config").
		NewFunctionBuilder().WithFunc(func(_ context.Context, m api.Module, _ uint32, ptr, size uint32) {
		msg, _ := m.Memory().Read(ptr, size)
		c.logs = append(c.logs, string(msg))
	}).Export("log").
		NewFunctionBuilder().WithFunc(func(_ context.Context, m api.Module, ptr, size uint32) int32 {
		payload, ok := m.Memory().Read(ptr, size)
		if !ok {
			return -1
		}
		c.results = append(c.results, append([]byte(nil), payload...))
		return 0
	}).Export("submit_result").
		NewFunctionBuilder().WithFunc(func(_ context.Context, _ api.Module, _, _ uint32) int32 {
		return 0
	}).Export("emit_telemetry").
		Instantiate(ctx)
	return err
}

// runCheck mirrors go/pkg/agent/plugin_runtime_wazero.go in the ServiceRadar
// repository.
func runCheck(t *testing.T, wasmPath string, config []byte) (*capture, error) {
	t.Helper()
	wasm, err := os.ReadFile(wasmPath)
	if err != nil {
		t.Fatalf("read %s: %v", wasmPath, err)
	}

	ctx := context.Background()
	rt := wazero.NewRuntime(ctx)
	defer rt.Close(ctx)

	if _, err := wasi_snapshot_preview1.Instantiate(ctx, rt); err != nil {
		t.Fatalf("instantiate wasi: %v", err)
	}
	c := &capture{config: config}
	if err := c.instantiateEnv(ctx, rt); err != nil {
		t.Fatalf("instantiate env: %v", err)
	}

	modConfig := wazero.NewModuleConfig().
		WithName("plugin").
		WithSysWalltime().
		WithSysNanotime().
		WithSysNanosleep().
		WithStartFunctions()

	mod, err := rt.InstantiateWithConfig(ctx, wasm, modConfig)
	if err != nil {
		t.Fatalf("instantiate plugin: %v", err)
	}
	fn := mod.ExportedFunction("run_check")
	if fn == nil {
		t.Fatal("plugin does not export run_check")
	}
	_, err = fn.Call(ctx)
	return c, err
}

func TestWasip1ClockAndSleepWorkUnderAgentRuntime(t *testing.T) {
	path := os.Getenv("CLOCK_CHECK_WASM_WASIP1")
	if path == "" {
		t.Skip("CLOCK_CHECK_WASM_WASIP1 not set; build examples/clock-check for wasm32-wasip1 first")
	}

	c, err := runCheck(t, path, []byte(`{"sleep_ms":25}`))
	if err != nil {
		t.Fatalf("run_check trapped: %v (logs: %v)", err, c.logs)
	}
	if len(c.results) != 1 {
		t.Fatalf("expected one submitted result, got %d (logs: %v)", len(c.results), c.logs)
	}

	var result struct {
		Status  string            `json:"status"`
		Summary string            `json:"summary"`
		Labels  map[string]string `json:"labels"`
	}
	if err := json.Unmarshal(c.results[0], &result); err != nil {
		t.Fatalf("decode result: %v: %s", err, c.results[0])
	}
	if !strings.HasPrefix(result.Summary, "slept ") {
		t.Fatalf("unexpected summary %q", result.Summary)
	}
	elapsed := result.Labels["elapsed_ms"]
	if elapsed == "" || elapsed == "0" {
		t.Fatalf("monotonic clock did not advance across sleep: labels=%v", result.Labels)
	}
	var ms int
	for _, ch := range elapsed {
		ms = ms*10 + int(ch-'0')
	}
	if ms < 25 {
		t.Fatalf("sleep returned early: elapsed_ms=%d, want >= 25", ms)
	}
}

func TestUnknownUnknownClockTrapsUnderAgentRuntime(t *testing.T) {
	path := os.Getenv("CLOCK_CHECK_WASM_UNKNOWN")
	if path == "" {
		t.Skip("CLOCK_CHECK_WASM_UNKNOWN not set; build examples/clock-check for wasm32-unknown-unknown first")
	}

	c, err := runCheck(t, path, []byte(`{"sleep_ms":25}`))
	if err == nil && len(c.results) == 1 {
		t.Fatal("wasm32-unknown-unknown build ran its clock and sleep; expected a trap, which is why the SDK targets wasm32-wasip1")
	}
	var exitErr interface{ ExitCode() uint32 }
	if err != nil && errors.As(err, &exitErr) {
		t.Logf("exited with code %d", exitErr.ExitCode())
	}
}
