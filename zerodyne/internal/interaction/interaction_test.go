package interaction

import (
	"context"
	"testing"
)

type stub struct {
	name string
}

func (s stub) Name() string  { return s.name }
func (s stub) Usage() string { return s.name + " ..." }
func (s stub) Exec(_ context.Context, args []string) (Result, error) {
	return Result{Message: s.name + ":" + string(rune('0'+len(args)))}, nil
}

func TestRegistryDispatch(t *testing.T) {
	r := NewRegistry()
	r.MustRegister(stub{"b"})
	r.MustRegister(stub{"a"})
	if got := r.Names(); len(got) != 2 || got[0] != "a" {
		t.Fatalf("names = %v", got)
	}
	res, err := r.Exec(context.Background(), "b", []string{"x", "y"})
	if err != nil || res.Message != "b:2" {
		t.Fatalf("exec = %+v, %v", res, err)
	}
	if _, err := r.Exec(context.Background(), "nope", nil); err == nil {
		t.Fatal("expected unknown-operation error")
	}
}

func TestRegistryDuplicate(t *testing.T) {
	r := NewRegistry()
	r.MustRegister(stub{"a"})
	if err := r.Register(stub{"a"}); err == nil {
		t.Fatal("expected duplicate error")
	}
}
