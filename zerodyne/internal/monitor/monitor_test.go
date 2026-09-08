package monitor

import (
	"context"
	"testing"
)

type stub struct {
	Base
	name string
	data any
}

func (s stub) Name() string                          { return s.name }
func (s stub) Snapshot(context.Context) (any, error) { return s.data, nil }

func TestRegistryDispatch(t *testing.T) {
	r := NewRegistry()
	r.MustRegister(stub{name: "b", data: "bee"})
	r.MustRegister(stub{name: "a", data: "aye"})

	if got := r.Names(); len(got) != 2 || got[0] != "a" || got[1] != "b" {
		t.Fatalf("names = %v", got)
	}
	v, err := r.Snapshot(context.Background(), "b")
	if err != nil || v != "bee" {
		t.Fatalf("snapshot = %v, %v", v, err)
	}
	if _, err := r.Snapshot(context.Background(), "nope"); err == nil {
		t.Fatal("expected unknown-target error")
	}
}

func TestRegistryDuplicate(t *testing.T) {
	r := NewRegistry()
	r.MustRegister(stub{name: "a"})
	if err := r.Register(stub{name: "a"}); err == nil {
		t.Fatal("expected duplicate error")
	}
	if err := r.Register(stub{}); err == nil {
		t.Fatal("expected unnamed error")
	}
}
