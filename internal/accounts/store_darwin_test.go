//go:build darwin

package accounts

import (
	"bytes"
	"context"
	"os"
	"strings"
	"sync"
	"testing"
)

// sentinelUUID is a recognisable prefix used in live keychain tests so
// cleanup can force-delete items even if the test panics.
const sentinelUUID = "aistat-test-00000000-0000-0000-0000-000000000001"

// skipUnlessLive skips the test when AISTAT_LIVE_KEYCHAIN is not set to "1".
func skipUnlessLive(t *testing.T) {
	t.Helper()
	if os.Getenv("AISTAT_LIVE_KEYCHAIN") != "1" {
		t.Skip("set AISTAT_LIVE_KEYCHAIN=1 to run live keychain tests")
	}
}

// forceDeleteSentinel removes the sentinel per-account keychain item and
// its index entry regardless of error. Used in t.Cleanup to avoid leaking
// test items into the user's keychain.
func forceDeleteSentinel(uuid string) {
	ctx := context.Background()
	svc := darwinServicePrefix + uuid
	darwinDeleteItem(ctx, svc, "")
	// Best-effort index clean: open store and delete. Ignore errors.
	if s, err := OpenStore(); err == nil {
		s.Delete(ctx, uuid)
	}
}

func TestDarwinStore_AddListDelete(t *testing.T) {
	skipUnlessLive(t)

	t.Cleanup(func() { forceDeleteSentinel(sentinelUUID) })

	s, err := OpenStore()
	if err != nil {
		t.Fatalf("OpenStore: %v", err)
	}
	ctx := context.Background()

	a := makeTestAccount(sentinelUUID, "aistat-test@example.com")

	if err := s.Upsert(ctx, a); err != nil {
		t.Fatalf("Upsert: %v", err)
	}

	list, err := s.List(ctx)
	if err != nil {
		t.Fatalf("List: %v", err)
	}
	found := false
	for _, acct := range list {
		if acct.UUID == sentinelUUID {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("sentinel UUID not found in List result")
	}

	if err := s.Delete(ctx, sentinelUUID); err != nil {
		t.Fatalf("Delete: %v", err)
	}

	list, err = s.List(ctx)
	if err != nil {
		t.Fatalf("List after delete: %v", err)
	}
	for _, acct := range list {
		if acct.UUID == sentinelUUID {
			t.Errorf("sentinel UUID still present after delete")
		}
	}
}

func TestDarwinStore_OrphanIndexHandling(t *testing.T) {
	skipUnlessLive(t)

	orphanUUID := "aistat-test-orphan-0000-0000-0000-000000000002"
	liveUUID := "aistat-test-live-0000-0000-0000-000000000003"
	t.Cleanup(func() {
		forceDeleteSentinel(orphanUUID)
		forceDeleteSentinel(liveUUID)
	})

	s, err := OpenStore()
	if err != nil {
		t.Fatalf("OpenStore: %v", err)
	}
	ctx := context.Background()

	// Upsert the live account normally.
	liveAcct := makeTestAccount(liveUUID, "live@example.com")
	if err := s.Upsert(ctx, liveAcct); err != nil {
		t.Fatalf("Upsert live: %v", err)
	}

	// Manually write an orphan UUID into the index without creating the
	// per-account item, simulating a crash between index-update and item-write
	// on a Delete.
	ds := s.(*darwinStore)
	if err := ds.withLock(func() error {
		uuids, err := ds.readIndex(ctx)
		if err != nil {
			return err
		}
		uuids = append(uuids, orphanUUID)
		return ds.writeIndex(ctx, uuids)
	}); err != nil {
		t.Fatalf("inject orphan: %v", err)
	}

	// List should skip the orphan entry and return only the live account.
	var debugBuf bytes.Buffer
	sWithDebug, err := OpenStore(WithDebug(&debugBuf))
	if err != nil {
		t.Fatalf("OpenStore with debug: %v", err)
	}
	list, err := sWithDebug.List(ctx)
	if err != nil {
		t.Fatalf("List: %v", err)
	}
	if len(list) != 1 || list[0].UUID != liveUUID {
		t.Errorf("want [%s], got %v", liveUUID, list)
	}
	if !strings.Contains(debugBuf.String(), orphanUUID) {
		t.Errorf("expected orphan warn for %s in debug output; got: %q", orphanUUID, debugBuf.String())
	}
}

func TestDarwinStore_ConcurrentUpserts(t *testing.T) {
	skipUnlessLive(t)

	uuid1 := "aistat-test-conc1-0000-0000-0000-000000000004"
	uuid2 := "aistat-test-conc2-0000-0000-0000-000000000005"
	t.Cleanup(func() {
		forceDeleteSentinel(uuid1)
		forceDeleteSentinel(uuid2)
	})

	s, err := OpenStore()
	if err != nil {
		t.Fatalf("OpenStore: %v", err)
	}
	ctx := context.Background()

	a1 := makeTestAccount(uuid1, "conc1@example.com")
	a2 := makeTestAccount(uuid2, "conc2@example.com")

	var wg sync.WaitGroup
	wg.Add(2)
	go func() { defer wg.Done(); s.Upsert(ctx, a1) }()
	go func() { defer wg.Done(); s.Upsert(ctx, a2) }()
	wg.Wait()

	list, err := s.List(ctx)
	if err != nil {
		t.Fatalf("List: %v", err)
	}
	found := map[string]bool{}
	for _, a := range list {
		found[a.UUID] = true
	}
	if !found[uuid1] || !found[uuid2] {
		t.Errorf("want both UUIDs present after concurrent upserts; got: %v", list)
	}
}
