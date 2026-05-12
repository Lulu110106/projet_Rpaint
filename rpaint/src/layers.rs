use crate::model::Shape;

/// Représente un layer individuel dans le système de layers
#[derive(Clone)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub elements: Vec<Shape>,
}

impl Layer {
    /// Crée un nouveau layer vide avec un ID et un nom
    pub fn new(id: u64, name: String) -> Self {
        Layer {
            id,
            name,
            visible: true,
            elements: Vec::new(),
        }
    }

    /// Crée le layer initial par défaut
    pub fn default_layer() -> Self {
        Layer {
            id: 1,
            name: "Base Layer".to_string(),
            visible: true,
            elements: Vec::new(),
        }
    }

    /// Ajoute un élément au layer
    pub fn add_element(&mut self, shape: Shape) {
        self.elements.push(shape);
    }

    /// Clone le layer pour les opérations undo/redo
    pub fn clone_for_undo(&self) -> Layer {
        Layer {
            id: self.id,
            name: self.name.clone(),
            visible: self.visible,
            elements: self.elements.clone(),
        }
    }
}

/// Gestionnaire des layers pour centraliser la logique métier
pub struct LayerManager {
    pub layers: Vec<Layer>,
    pub active_layer_id: u64,
}

impl LayerManager {
    /// Crée un gestionnaire avec un layer par défaut
    pub fn new() -> Self {
        let default_layer = Layer::default_layer();
        LayerManager {
            layers: vec![default_layer],
            active_layer_id: 1,
        }
    }

    /// Retourne le layer actif
    pub fn get_active_layer(&self) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == self.active_layer_id)
    }

    /// Retourne le layer actif mutable
    pub fn get_active_layer_mut(&mut self) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == self.active_layer_id)
    }

    /// Retourne le layer par ID
    pub fn get_layer(&self, id: u64) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Retourne le layer par ID mutable
    pub fn get_layer_mut(&mut self, id: u64) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// Retourne l'index du layer par ID
    pub fn get_layer_index(&self, id: u64) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    /// Crée un nouveau layer vide à une position donnée
    pub fn create_layer_at(&mut self, id: u64, name: String, position: usize) {
        let layer = Layer::new(id, name);
        let insert_position = position.min(self.layers.len());
        self.layers.insert(insert_position, layer);
        self.active_layer_id = id;
    }

    /// Supprime un layer par ID (avec validation: ne pas supprimer le dernier)
    pub fn delete_layer(&mut self, id: u64) -> bool {
        if self.layers.len() <= 1 {
            return false; // Ne pas supprimer le dernier layer
        }

        if let Some(idx) = self.get_layer_index(id) {
            self.layers.remove(idx);

            // Si le layer supprimé était actif, sélectionner un autre
            if self.active_layer_id == id {
                if let Some(remaining_layer) = self.layers.first() {
                    self.active_layer_id = remaining_layer.id;
                }
            }
            return true;
        }
        false
    }

    /// Sélectionne un layer actif
    pub fn set_active_layer(&mut self, id: u64) -> bool {
        if self.layers.iter().any(|l| l.id == id) {
            self.active_layer_id = id;
            true
        } else {
            false
        }
    }

    /// Change la visibilité d'un layer
    pub fn set_layer_visibility(&mut self, id: u64, visible: bool) {
        if let Some(layer) = self.get_layer_mut(id) {
            layer.visible = visible;
        }
    }

    /// Renomme un layer
    pub fn rename_layer(&mut self, id: u64, new_name: String) -> bool {
        if let Some(layer) = self.get_layer_mut(id) {
            layer.name = new_name;
            true
        } else {
            false
        }
    }

    /// Réorganise les layers (déplace un layer à une nouvelle position)
    pub fn reorder_layer(&mut self, from_idx: usize, to_idx: usize) {
        if from_idx < self.layers.len() && to_idx < self.layers.len() && from_idx != to_idx {
            let layer = self.layers.remove(from_idx);
            self.layers.insert(to_idx, layer);
        }
    }

    /// Retourne tous les éléments visibles (pour le rendu)
    /// Les éléments sont retournés du fond vers le sommet
    pub fn get_visible_elements(&self) -> Vec<&Shape> {
        let mut elements = Vec::new();
        // Parcourir du dernier layer (fond) au premier (sommet)
        for layer in self.layers.iter().rev() {
            if layer.visible {
                for element in &layer.elements {
                    elements.push(element);
                }
            }
        }
        elements
    }

}
