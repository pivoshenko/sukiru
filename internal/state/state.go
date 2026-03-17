package state

import (
	"encoding/json"
	"os"
	"path/filepath"
)

type SkillEntry struct {
	Destination    string `json:"destination"`
	Hash           string `json:"hash"`
	Skill          string `json:"skill"`
	Source         string `json:"source"`
	SourceRevision string `json:"source_revision"`
	UpdatedAt      string `json:"updated_at"`
}

type State struct {
	Version int                   `json:"version"`
	LastRun string                `json:"last_run"`
	Skills  map[string]SkillEntry `json:"skills"`
}

func defaultStatePath() string {
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".ai", "bootstrap", "state.json")
}

func LoadDefault() (State, error) {
	return Load(defaultStatePath())
}

func SaveDefault(s State) error {
	return Save(defaultStatePath(), s)
}

func Load(path string) (State, error) {
	s := State{Version: 1, Skills: map[string]SkillEntry{}}
	b, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return s, nil
		}
		return s, err
	}
	if err := json.Unmarshal(b, &s); err != nil {
		return State{Version: 1, Skills: map[string]SkillEntry{}}, nil
	}
	if s.Skills == nil {
		s.Skills = map[string]SkillEntry{}
	}
	return s, nil
}

func Save(path string, s State) error {
	if s.Skills == nil {
		s.Skills = map[string]SkillEntry{}
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	b, _ := json.MarshalIndent(s, "", "  ")
	return os.WriteFile(path, b, 0o644)
}
