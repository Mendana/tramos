-- 0011: competición o entrenamiento (#195). Ver docs/almacenamiento.md y docs/modelo.md
-- ("Competición o entrenamiento").
--
-- Las carreras guardadas antes son competiciones.
ALTER TABLE events ADD COLUMN kind TEXT NOT NULL DEFAULT 'race' CHECK (kind IN ('race', 'training'));
